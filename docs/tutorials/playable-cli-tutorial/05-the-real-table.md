# Chapter 05 — The real table

The study table is the terminal front-end of the Gomoku workspace: a
rendered board with a side panel, driven by the command language of
chapters 01–04. Two seats, human or AI in any combination, share one
persistent variation tree, and the Swap2 opening runs as a phase of
the session. This chapter builds the table from the headless core
outward, one concern per sub-chapter.

## Architecture

```text
main.rs          arguments ──► SessionInit ──► study::run(init)
                                                  │
                 ┌────────────────────────────────┘
                 ▼
tui/study.rs     the loop: read one input ──► command::parse ──► SessionCore::execute
                 │                                                     │
                 ▼                                                     ▼
tui/screen.rs    Screen::draw(frame) ◄── render.rs (pure) ◄── events
                 │
        ┌────────┴─────────┐
        ▼                  ▼
  AltScreen           LineScreen
  (crossterm,         (println!,
   RAII guard)         the fallback)
```

The dependency rule: rendering depends on the session, the shell
depends on rendering, never the other direction. Crossterm appears
only inside `src/tui/`. The session core stays headless.

## The sub-chapters

Each sub-chapter has its own tests and its own solution file, kept
next to the tutorial (`refactor-solution.md`, and so on).

| sub-chapter | what it teaches |
|---|---|
| [a — Refactor](05-the-real-table/a-refactor/refactor.md) | Extraction without behavior change: one notation module, one repl core, one shared test-kit crate. |
| [b — The renderer](05-the-real-table/b-renderer/renderer.md) | Rendering as a pure function: session state in, lines out. |
| [c — Swap2 as a session phase](05-the-real-table/c-swap2/swap2.md) | The opening as session state: a replay log with the engine's typestate as its validated projection. |
| [d — The alternate-screen shell](05-the-real-table/d-shell/shell.md) | The TUI as a renderer plus an input-translation layer: modal input, an RAII guard, one loop. |
| [e — The line-mode fallback](05-the-real-table/e-fallback/fallback.md) | A fallback as a second implementation of a small trait, not a second copy of the loop. |
| [f — The binary](05-the-real-table/f-binary/binary.md) | Wiring and the user-facing surface: the clap front-end, validation, the README, and the capstone tests. |

## Background documents

* [00-design.md](05-the-real-table/00-design.md) — the design paper:
  the contract for the six sub-chapters and the record of the decided
  questions.
* [00-tui-excursion.md](05-the-real-table/00-tui-excursion.md) — the
  crossterm side track: raw mode, the alternate screen, the RAII
  guard, and the loop shape. Required reading before sub-chapter d.

Next: [Chapter 06 — Seeing the mind](06-seeing-the-mind.md)
