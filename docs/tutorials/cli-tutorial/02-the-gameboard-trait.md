# Chapter 2 — The GameBoard trait

Goal: every line of UI code talks to `&dyn GameBoard`, never to a
concrete board. The engine choice happens exactly once, at startup,
behind a factory function.

This is the chapter your question was about — so first, the ground
rules that make `dyn` work *at all*.



## Rust toolbox

**Object safety.** A trait can become `dyn Trait` only if every
method is *dispatchable without knowing the concrete type*:

- no generic method (`fn foo<T>(&self, ...)`) — the vtable can't hold
  infinitely many instantiations
- no `Self` in argument or return position (`fn new() -> Self` would
  return *what*, through a vtable?)
- no associated constants

Consequence: **constructors never live on object-safe traits.** You
construct concrete types behind a factory that returns
`Box<dyn GameBoard>` — then everything else is virtual.

**The orphan rule, working for you.** You may `impl GameBoard for
engine::Board` in the CLI crate because *the trait is yours* (one of
the two must be local). The engine crate stays exactly as chapter 13
wants it: trait-free, statically dispatched.

**Newtype adapters.** Instead of implementing the trait directly on
the foreign types, wrap them: `struct FastBoard(engine::Board)`. Same
effect, two advantages: the wrapper can hold UI-side extras later
(highlight state, a name), and there's no method-name ambiguity
between the inherent API and the trait.

**What `dyn` costs.** A `Box<dyn GameBoard>` is a fat pointer: data
pointer + vtable pointer. Each call is one indirect jump, and the
compiler can't inline across it. At human typing speed this is
unmeasurable; at self-play speed it would matter — which is the whole
reason the trait lives here and not in the engine.

## Contract

`crates/cli/src/board.rs`:

```rust
/// Everything the UI needs from a board, and nothing more.
pub trait GameBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    fn status(&self) -> Status;
    fn to_move(&self) -> Color;
    fn stone_at(&self, mv: Move) -> Option<Color>;
    fn moves(&self) -> &[Move];
    fn name(&self) -> &'static str;
}

pub struct NaiveBoard(engine::reference::Board);
pub struct FastBoard(engine::Board);

#[derive(Debug, Clone, Copy)]
pub enum EngineKind { Naive, Fast }

pub fn board_for(kind: EngineKind) -> Box<dyn GameBoard>;
```

Both `impl GameBoard for …` blocks are one-line forwards — that's the
point. If you find yourself writing logic in an adapter, the
abstraction is leaking.

Note what is **not** on the trait: `new` (not object-safe — the factory
handles it) and `is_legal` (the UI plays optimistically and shows the
`PlayError`; friendlier, and one less method to agree on).

`undo` is a *scope* decision rather than a limitation: **both** boards
have it (the oracle replays the history minus the last move; the bitboard
board clears the bit and re-opens the game), so the trait could carry it —
and [chapter 6](06-polish.md) adds it as an exercise. Until then the core
trait stays as small as the specified UI needs. One contract to know when
you do add it: both implementations **panic on an empty history**, so any
UI path must check `moves().is_empty()` first.

## The test that sells it

Playing through `Box<dyn GameBoard>` on *both* boards and demanding
identical results is the differential test of tutorial 13, seen from
the consumer side:

```rust
#[test]
fn both_boards_agree_through_the_trait() {
    let mut boards: Vec<Box<dyn GameBoard>> =
        vec![board_for(EngineKind::Naive), board_for(EngineKind::Fast)];
    let script = [(7u8, 7u8), (7, 8), (8, 7), (8, 8), (6, 7)];
    for (r, c) in script {
        let mv = Move::new(r, c).unwrap();
        for b in &mut boards {
            assert!(b.play(mv).is_ok());
        }
        let [a, b] = &mut boards[..] else { unreachable!() };
        assert_eq!(a.status(), b.status());
        assert_eq!(a.stone_at(mv), b.stone_at(mv));
    }
}
```

## Steps

1. Write `board.rs` against the contract (trait, wrappers, factory).
2. In `main.rs`: pick an engine by name from `--engine naive|fast`
   (default `fast`), build it via `board_for`, print its `name()`,
   `status()`, and `to_move()` — *through* the trait object.
3. Wire the test above; run it.

## Pitfalls

- `render(&board)` where `board: Box<dyn GameBoard>` doesn't coerce
  to `&dyn GameBoard` everywhere you might expect — write `&*board`
  (deref the box, re-borrow as a trait object). Chapter 3 needs this.
- Don't be tempted to move `GameBoard` into the engine crate "for
  reuse". The day `mcts` needs a board abstraction it will want
  different methods (undo, keys) — let each consumer own its seam.

## Done when

`cargo run -p cli -- --engine naive` and `-- --engine fast` both
print their banner, and the agreement test is green.

Next: [Chapter 3 — Rendering](03-rendering.md)

---

## Solution

### `crates/cli/src/board.rs`

```rust
//! The UI's view of a Gomoku board — object-safe, engine-agnostic.
//!
//! This trait lives HERE, not in the engine: the CLI is the consumer
//! that needs runtime board selection. The engine keeps static
//! dispatch for self-play speed (ch. 13, "no shared trait").

use engine::{Color, Move, PlayError, Status};

/// Everything the UI needs from a board, and nothing more.
///
/// Object-safe by design: no generics, no `Self` returns, no
/// constructors. Build concrete boards behind [`board_for`].
pub trait GameBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError>;
    fn status(&self) -> Status;
    fn to_move(&self) -> Color;
    fn stone_at(&self, mv: Move) -> Option<Color>;
    fn moves(&self) -> &[Move];
    fn name(&self) -> &'static str;
}

/// The naive oracle, wrapped so UI extras have a home.
pub struct NaiveBoard(engine::reference::Board);

/// The production bitboard engine.
pub struct FastBoard(engine::Board);

impl GameBoard for NaiveBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }
    fn status(&self) -> Status {
        self.0.status()
    }
    fn to_move(&self) -> Color {
        self.0.to_move()
    }
    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }
    fn moves(&self) -> &[Move] {
        self.0.moves()
    }
    fn name(&self) -> &'static str {
        "naive"
    }
}

impl GameBoard for FastBoard {
    fn play(&mut self, mv: Move) -> Result<(), PlayError> {
        self.0.play(mv)
    }
    fn status(&self) -> Status {
        self.0.status()
    }
    fn to_move(&self) -> Color {
        self.0.to_move()
    }
    fn stone_at(&self, mv: Move) -> Option<Color> {
        self.0.stone_at(mv)
    }
    fn moves(&self) -> &[Move] {
        self.0.moves()
    }
    fn name(&self) -> &'static str {
        "fast"
    }
}

/// Which engine board to play on.
#[derive(Debug, Clone, Copy)]
pub enum EngineKind {
    Naive,
    Fast,
}

/// The one place a concrete board type is chosen.
pub fn board_for(kind: EngineKind) -> Box<dyn GameBoard> {
    match kind {
        EngineKind::Naive => Box::new(NaiveBoard(engine::reference::Board::new())),
        EngineKind::Fast => Box::new(FastBoard(engine::Board::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_boards_agree_through_the_trait() {
        let mut boards: Vec<Box<dyn GameBoard>> =
            vec![board_for(EngineKind::Naive), board_for(EngineKind::Fast)];
        let script = [(7u8, 7u8), (7, 8), (8, 7), (8, 8), (6, 7)];
        for (r, c) in script {
            let mv = Move::new(r, c).unwrap();
            for b in &mut boards {
                assert!(b.play(mv).is_ok());
            }
            let [a, b] = &mut boards[..] else {
                unreachable!()
            };
            assert_eq!(a.status(), b.status());
            assert_eq!(a.stone_at(mv), b.stone_at(mv));
        }
    }

    #[test]
    fn play_error_travels_through_the_trait() {
        let mut board = board_for(EngineKind::Fast);
        let mv = Move::new(7, 7).unwrap();
        board.play(mv).unwrap();
        assert_eq!(board.play(mv), Err(PlayError::Occupied));
    }
}
```

### `crates/cli/src/main.rs`

```rust
mod board;

use board::{board_for, EngineKind};

fn main() {
    let kind = parse_engine_arg().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let board = board_for(kind);
    println!(
        "board: {} · {:?} to move · status {:?}",
        board.name(),
        board.to_move(),
        board.status()
    );
}

fn parse_engine_arg() -> Result<EngineKind, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref()) {
        (None, None) => Ok(EngineKind::Fast),
        (Some("--engine"), Some("naive")) => Ok(EngineKind::Naive),
        (Some("--engine"), Some("fast")) => Ok(EngineKind::Fast),
        (Some("--engine"), Some(other)) => Err(format!("unknown engine '{other}'")),
        (Some("--engine"), None) => Err("--engine needs a value: naive|fast".into()),
        (Some(other), _) => Err(format!("unknown argument '{other}' (want: --engine naive|fast)")),
        (None, Some(_)) => unreachable!(),
    }
}
```

Run both:

```text
$ cargo run -p cli -- --engine naive
board: naive · Black to move · status Ongoing
$ cargo run -p cli                      # default: fast
board: fast · Black to move · status Ongoing
```
