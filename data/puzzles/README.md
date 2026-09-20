# puzzles/ — VCF puzzle sets

Tactical puzzle collections for the milestone-4 anchor set. **All
files git-ignored** (data rights unclear — wrapper repo is MIT but
the puzzle content is third-party; see `data/README.md`). Regenerate
with `fetch.sh`.

## Content (`files/`)

| File | Records | Labelled? | Source |
|---|---|---|---|
| `vcf-material-763.json` | 763 | **yes — solution lines** | banbu-gomoku `public/puzzles/vcf-material.json` |
| `kaibao-vcf-1052.json` | 1,050 | no (positions only) | banbu-gomoku kaibao 实战VCF_1052题 |
| `renju-portal-vcf-4024.json` | 4,024 | no (positions only) | RenjuPortal collection via banbu-gomoku |

Downloaded 2026-09-20; all URLs HTTP 200 at download time.

## Format (vcf-material-763.json)

```json
{ "v": 1, "count": 763, "items": [
  { "i": 0, "a": "black", "d": 6, "b": [[x,y],...], "w": [[x,y],...],
    "l": [[x,y,side],...] } ] }
```

`a` = attacker (side with the forced win), `d` = line depth,
`b`/`w` = stone lists, `l` = the claimed forced-win line with side
markers. Coordinates are 0-based `x,y`.

## Fit and required processing

- `vcf-material-763`: the anchor-set seed. Every line must be
  **re-verified by our own `verify_line`** before use (ch. 14,
  decision D5; soundness gate unchanged) — and re-adjudicated under
  freestyle rules (the source has no explicit ruleset tag).
- The unlabelled sets are generator input for the TSS oracle
  (milestone 4): our prover produces the labels, `verify_line`
  filters them. This is the scalable path — no second labelled
  public set exists (findings report, negative results).

Further lead (not downloaded): the lfz084/renju puzzle archive —
thousands of Renju-oriented positions without answers:
https://github.com/lfz084/renju/tree/master/puzzle/json
