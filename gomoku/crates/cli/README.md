# `cli` — terminal Gomoku front-end

A human-vs-human terminal UI on the `engine` boards. It is deliberately
thin: the CLI owns rendering, input parsing, and the Swap2/puzzle
workflows; the engine owns rules and tactics.

## Build

```bash
cd gomoku
cargo build -p cli
```

The `naive-engine` feature is on by default; it pulls in the engine's
`testutil` reference board. Disable it for a lean build:

```bash
cargo build -p cli --no-default-features
```

## Hotseat play (Swap2 opening)

```bash
cargo run -p cli
```

Player A places two Black stones and one White stone. Player B then
chooses:

- `b` — play Black,
- `w` — play White, or
- `a` — add one Black and one White stone, after which Player A picks a
color.

After the opening the two parties alternate normally.

Keys (any phase):

| key | action |
|-----|--------|
| `<row> <col>` | play at 0-based coordinates, e.g. `7 7` |
| `u` | undo |
| `t` | toggle TSS (threat-space search) forced-win overlay |
| `new` | restart |
| `q` | quit |

Flags:

```bash
cargo run -p cli -- --engine naive    # use the reference engine
cargo run -p cli -- --engine fast     # default
cargo run -p cli -- --plain           # scrolling mode (debugging/piping)
cargo run -p cli -- --no-swap2        # start from an empty board
```

## Puzzle examination mode

```bash
cargo run -p cli -- --puzzle ../data/puzzles/files/vcf-material-763.json
```

Load a puzzle file and browse its samples. Each sample is displayed at
the puzzle position; you can play moves, undo, and redo from there.
Both front-ends are interactive: the default uses the alternate
screen; add `--plain` for the scrolling variant (debugging/piping).

### Sidecar convention (parser trust gate)

A puzzle data file may only be loaded if a sidecar file sits next to it.
For `foo.json` the sidecar is `foo.parser`. Its first non-`#`-comment
line names the parser that has been verified for that format. The gate
is deliberate: a format must be re-adjudicated before the CLI trusts it.

The sidecar for the verified material set is already present:

```text
data/puzzles/files/vcf-material-763.parser
```

### Registering a new parser

1. Verify the JSON schema and coordinate system of the new puzzle set.
2. Add a sidecar `foo.parser` containing the new parser identifier.
3. Implement the parser in `src/puzzle.rs` and add it to the match
   expression in `puzzle::load`.
4. Update this README with the new format.

Do not create sidecars for unverified formats.

### Puzzle keys

| key | action |
|-----|--------|
| `<row> <col>` | play a move from the puzzle position |
| `u` | undo (stops at the puzzle root) |
| `r` | redo |
| `n` | next puzzle (wraps around) |
| `p` | previous puzzle (wraps around) |
| `t` | toggle TSS overlay |
| `new` | reset the current puzzle |
| `q` | quit |

Flags:

```bash
cargo run -p cli -- --puzzle ../data/puzzles/files/vcf-material-763.json --index 5
```

`--index` is 0-based and defaults to 0.

## Coordinate system

All board input uses 0-based row and column: `0 0` is the top-left
corner, `7 7` is the centre. The `vcf-material-v1` file format stores
coordinates as `[x, y]` where `x` is the column and `y` is the row; the
parser maps them to `Move::new(y, x)`.
