# gomocup-games/ — Gomocup tournament game records (PSQ)

Gomocup is the annual Gomoku engine tournament; its result archives
contain every played game as a `.psq` file. This folder holds the
Freestyle15 subset — the only ruleset matching ours (15×15,
overlines win).

## Content

`games/2026/Freestyle15_1/` + `games/2026/Freestyle15_2/` — 12,984
games from the 2026 tournament (123 MB uncompressed, downloaded
2026-09-20). **Git-ignored** (local-only: no stated license — see
`data/README.md`).

## PSQ format

Plain text, one game per file:

```text
Piskvorky 15x15, 11:11, 0     <- header: board, tournament time, ?
14,14,0                        <- x,y,ms: move, then milliseconds used
12,12,0
...
```

Coordinates are 0-based `x,y` (column,row) — note the order differs
from our engine's `Move::new(row, col)`. Move order alternates,
Black first; Swap2 openings appear as their first 3–5 moves.

## Regenerating / more years

`fetch.sh [YEAR...]` downloads the archives from
https://gomocup.org/results/ and extracts the Freestyle15 games for
the given years (default: 2026). Total archive volume 2000–2026 is
~419 MB compressed.

## Fit

Supervised pretraining supplement (ch. 14 §5.3, phase 0) and
anchor-set root positions (milestone 4). Engine-strength labels:
these are games between top engines, not verified truth — ch. 14
§5.4 item 4 applies.
