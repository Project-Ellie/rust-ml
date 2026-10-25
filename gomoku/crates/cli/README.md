# `cli` — terminal Gomoku front-end

The `cli` crate is split into a headless library (variation-tree
history, command language, session core, seat actors, the Swap2
opening) and the thin `gomoku` binary. The terminal front-end is the
study table: two seats, human or AI in any combination, on a
persistent variation tree.

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

## Play at the study table

```bash
cargo run -p cli --bin gomoku                      # Swap2 opening, then the study table
cargo run -p cli --bin gomoku -- play              # same, spelled out
cargo run -p cli --bin gomoku -- play --no-swap2   # start from an empty board
cargo run -p cli --bin gomoku -- play --ai tactics # the AI takes White
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
| `play <square> [black\|white]` | place the next stone (the color token overrides the default alternation) |
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
| `b`, `w` | Swap2 placement: select the next stone's color; Swap2 choice: choose that color |
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
| `step` | let the AI play the side to move |
| `back [N]` / `forward [N]` | walk the variation tree |
| `var N` | choose child variation N |
| `take black\|white\|both` | make seat(s) human |
| `release black\|white\|both` | release seat(s) to the AI |
| `sims black\|white N` | set per-seat simulations |
| `new` | reset to the opening |
| `h`, `help`, `?` | show help |
| `q`, `quit`, `exit` | leave |

An empty line steps the AI when the side-to-move seat is AI.

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

When neither `--temperature` nor `--noise` is given, AI-vs-AI sessions
from an empty board use an opening schedule for the first ten plies
(temperature 1.0 with Dirichlet noise) to vary the opening. Passing
either flag explicitly disables the schedule.

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
