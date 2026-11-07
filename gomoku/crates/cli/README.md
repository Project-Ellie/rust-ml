# `cli` — terminal Gomoku front-end

## Abstract

The `cli` crate is split into a headless library (variation-tree
history, command language, session core, seat actors, the Swap2
opening) and the thin `gomoku` binary. The terminal front-end is the
study table: two seats, human or AI in any combination, on a
persistent variation tree.

## Glossary

- **Study**: a root board, variation tree, cursor, and seat configurations.
- **MCTS**: Monte Carlo tree search, which supplies candidate visits and priors.
- **PUCT**: the prior-guided exploration rule used by MCTS.
- **TSS**: threat-space search, the engine's bounded forced-win prover.
- **VCF**: victory by continuous fours, the forcing lines checked by the
  existing engine proof verifier.
- **Concession**: a historical resignation annotation, not an engine win
  or a proof that every legal move loses.

## Build

```bash
cd gomoku
cargo build -p cli
```

The `naive-engine` and `baseline` features are on by default; the
first pulls in the engine's `testutil` reference board, the second
the hand-written baseline evaluator. With the `baseline` feature
compiled in, the baseline is the default brain of AI sessions
(`--ai tactics` seats the mock instead). Disable the defaults for a
lean build:

```bash
cargo build -p cli --no-default-features
```

For real games against the full player, build release and play —
the baseline answers `release`/`selfplay` seats by default:

```bash
cargo build --release -p cli --features baseline
./target/release/gomoku play          # Swap2, then the study table
```

## Play at the study table

```bash
cargo run -p cli --bin gomoku                      # Swap2 opening, then the study table
cargo run -p cli --bin gomoku -- play              # same, spelled out
cargo run -p cli --bin gomoku -- play --no-swap2   # start from an empty board
cargo run -p cli --bin gomoku -- play --ai tactics # the AI takes White
cargo run -p cli --bin gomoku -- play --ai baseline # the full player takes White
cargo run -p cli --bin gomoku -- play --ai tactics --color white  # the AI takes Black
```

With no AI flag both seats are human. `--color` selects the human seat
and requires `--ai`.

### The Swap2 opening

Unless `--no-swap2` is given, the session opens with the Swap2
ceremony. Player A places three stones (two Black, one White). Player B
then chooses a color or declines with `more`, places one Black and one
White stone, and Player A chooses. The finished opening becomes the
root of the variation tree; White moves first.

Ceremony commands:

| command | action |
|---------|--------|
| `play <square>` | place the next stone (colors alternate, black first) |
| `back` | remove the last placement |
| `done` | end the current placement round |
| `black` / `white` | choose that color at a choice phase |
| `more` | decline the first choice and place two more stones |

### The full-screen shell

On a terminal the table runs full-screen with modal input. In normal
mode single keys act:

| key | action |
|-----|--------|
| arrows, `hjkl` | move the board cursor |
| Enter | play at the cursor (human seat), step the AI (AI seat); during Swap2 placement: place the next stone |
| `u` | undo: remove a Swap2 placement / go back one ply |
| `d` | Swap2 placement: end the round |
| `b`, `w` | Swap2 choice: choose that color (no effect during placement) |
| `m` | Swap2 first choice: place two more stones |
| `:` | open the command line (Esc cancels, Enter executes) |
| `?` | show the help screen |
| `q` | quit |

The `:` command line accepts the full typed language below.

### Study commands

The typed command language works on the `:` command line and in line
mode:

| command | action |
|---------|--------|
| `<square>` or `play <square>` | play a move, e.g. `h8` |
| `step` | let the AI play or, with `--resign`, concede for the side to move |
| `back [N]` / `forward [N]` | walk the variation tree |
| `var N` | choose child variation N |
| `take black\|white\|both` | make seat(s) human |
| `release black\|white\|both` | release seat(s) to the AI |
| `sims black\|white N` | set per-seat simulations |
| `new` | reset to the opening |
| `save PATH` / `load PATH` | save or replace a standalone study |
| `h`, `help`, `?` | show help |
| `q`, `quit`, `exit` | leave |

An empty line steps the AI when the side-to-move seat is AI and the
position is unfinished.

### Saving and loading

`save PATH` writes the full variation tree and cursor, root stones, seats,
AI defaults, simulation overrides, and optional launch seed. `load PATH`
validates the complete study before replacing the session; a failed load
leaves the session and overlays unchanged. Both refuse an unfinished
Swap2 ceremony. Paths are literal trimmed command remainders: spaces are
retained, but quotes, `~`, variables, and shell escapes are not expanded.

The writer emits native study **version 2**, including optional concession
annotations on every tree node and AI resignation settings. The reader
accepts versions 1 and 2 only. Old version-1 records load with missing
resignation settings disabled and annotations absent; version 1 cannot
contain enabled resignation settings or annotations. Part-1-only readers
reject version 2 rather than dropping its semantics.

Search statistics, proofs, puzzle-pack context, display state, and random
generator state are not saved. Loading reseeds from the saved launch seed
(or fresh randomness if absent), not from the point at which play stopped.
Loaded `new` returns to the loaded root with fresh unannotated history.

### Line mode

When stdout is not a terminal — a pipe, CI, a redirect — the same
session runs as plain text: each frame is printed below the previous
one and complete lines are read. Scripts drive the same session as a
terminal:

```bash
printf 'h8\nq\n' | cargo run -q -p cli --bin gomoku -- play --no-swap2
```

End of input quits the session.

### Search flags

`play` and `selfplay` share the search flags:

| flag | default | meaning |
|------|---------|---------|
| `--sims N` | 400 | MCTS simulations per move |
| `--c-puct F` | 1.5 | PUCT exploration constant |
| `--temperature F` | 0.0 | move-selection temperature |
| `--noise F` | 0.0 | Dirichlet noise weight |
| `--black-sims N` | — | override the Black seat's simulations |
| `--white-sims N` | — | override the White seat's simulations |
| `--seed N` | random | seed for reproducible sessions |
| `--resign` | false | opt in to heuristic top-five concession for either evaluator |

When neither `--temperature` nor `--noise` is given, AI-vs-AI sessions
from an empty board use an opening schedule for the first ten plies
(temperature 1.0 with Dirichlet noise) to vary the opening. Passing
either flag explicitly disables the schedule. `--resign` does not disable
or otherwise change this schedule; it reaches both initial AI seats and
seats later assigned with `release`.

### Optional heuristic resignation

After a real search, before noise or sampling, the player ranks candidates
by visits, then prior, then move index. With `--resign`, it checks at most
five top candidates (all if fewer than five empty cells remain). Each
successor must have an opponent VCF certificate accepted by the existing
engine verifier. Each probe uses the existing 1,000-node / 10-ply guard
budget; unknown or exhausted results cancel the concession. An own
immediate win also cancels it, and the existing certified-own-win bypass
remains unchanged.

This is a **heuristic**, not a global loss proof: other moves are unchecked
and might save the game. The displayed outcome states the candidate count
and this limitation. Concession annotates the current node without adding
a stone, advancing the cursor, or changing engine status. The real search
statistics remain available for policy and value display; TSS display is
disabled at finished nodes.

At a conceded node, Enter, Space, and an empty line do nothing; typed
`play` and `step` fail. No command overrides the concession there.
`back`, `forward`, and `var` remain available, including navigation into
already-existing children or siblings. Returning to the annotated node
restores its concession. `new` and puzzle navigation create fresh roots.

## Self-play

```bash
cargo run -p cli --bin gomoku -- selfplay --sims 400 --seed 42
```

Both seats are AI; press Enter to step. `--color` is not valid here:
no human seat exists.

## Headless demos

```bash
cargo run -p cli --bin historydemo    # scripted walk through the variation tree
cargo run -p cli --bin sessiondemo    # headless REPL over the session core
```

## Coordinate system

All board input uses algebraic notation: columns `a`–`o` (left to
right), rows `1`–`15` (top to bottom); `h8` is the centre.

## References

- [Player decision pipeline](../player/src/lib.rs) and
  [concession admission](../player/src/resign.rs).
- [Engine TSS prover and proof verifier](../engine/src/tss.rs).
- [Native study validation](src/session/record.rs) and
  [history records](src/history/record.rs).
