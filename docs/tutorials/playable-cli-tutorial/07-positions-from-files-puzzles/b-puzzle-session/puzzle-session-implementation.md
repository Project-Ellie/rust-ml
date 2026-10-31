# Sub-chapter 07b — The puzzle enrolls: implementation plan

This document turns the walkthrough in `puzzle-session.md` into a
construction order. It names the files, structs, functions, and tests.
It states behavioral contracts in prose. It does not contain bodies.

Read `puzzle-session.md` first. Work one step at a time. Write tests
before code. Open `puzzle-session-solution.md` only if you are stuck
for more than twenty minutes, or after the sub-chapter is complete.

---

## Step 0 — `session.rs`: the puzzle context

### 0.1 Add the `puzzle` import

In `src/session.rs`, import `Puzzle`, `PuzzleError`, and `PuzzlePack`
from `crate::puzzle`. Mark the import with
`#[allow(unused_imports)]` because `Puzzle` is not referenced until
sub-chapter 07c.

### 0.2 Add `PathBuf` to the imports

Add `use std::path::PathBuf;`.

### 0.3 Extend `SessionInit`

Add a public field:

```rust
pub puzzle: Option<PuzzleStudy>,
```

### 0.4 Add `PuzzleStudy`

Add a public struct:

```rust
pub struct PuzzleStudy {
    pub pack_path: PathBuf,
    pub index: usize,
    pub pack: PuzzlePack,
}
```

Behavior: the whole pack stays in memory so later navigation needs no
I/O. `pack_path` is only for display and error messages.

### 0.5 Add `SessionInit::puzzle`

Add a constructor:

```rust
pub fn puzzle(
    pack_path: PathBuf,
    pack: PuzzlePack,
    index: usize,
) -> Result<Self, PuzzleError>
```

Behavior:
* Validate `index` with `pack.get(index)`.
* Clone the puzzle board as the opening.
* Build a `SessionInit` with both seats human, default AI config, no
  seed, the puzzle board as `opening`, `swap2: false`, and `puzzle`
  set to `Some(PuzzleStudy { pack_path, index, pack })`.

### 0.6 Set `puzzle: None` in the existing constructors

Update `hotseat`, `hotseat_from_opening`, `versus_ai`, and
`ai_vs_ai` so that every `SessionInit` literal includes `puzzle: None`.

### 0.7 Extend `SessionCore`

Add a private field:

```rust
puzzle: Option<PuzzleStudy>,
```

### 0.8 Initialize the core puzzle field

In `SessionCore::new`, copy `init.puzzle.clone()` into `self.puzzle`.

### 0.9 Add `SessionCore::puzzle`

Add a public accessor:

```rust
pub fn puzzle(&self) -> Option<&PuzzleStudy>
```

Behavior: return `self.puzzle.as_ref()`.

### 0.10 Update `reset`

In `reset()`, after restoring seats, config, and sims, restore
`self.puzzle = self.init.puzzle.clone()`.

### 0.11 Gate

Run `cargo test -p cli --lib session::tests`.

---

## Step 1 — Ripple `puzzle: None` through struct literals

### 1.1 `src/main.rs`

Add `puzzle: None` to the `SessionInit` literals in `play_init` and
`selfplay_init`.

### 1.2 `tests/actor.rs`

Add `puzzle: None` to every explicit `SessionInit` struct literal.

### 1.3 `tests/study.rs`

Add `puzzle: None` to the explicit `SessionInit` struct literal.

### 1.4 Gate

Run `cargo check -p cli`.

---

## Step 2 — `tests/session.rs`: puzzle session tests

### 2.1 Import the puzzle module

Add `use cli::puzzle;`.

### 2.2 Add helpers

Add:

```rust
fn mini_pack() -> puzzle::PuzzlePack
```

Behavior: parse `tests/fixtures/mini-pack.json` with `include_str!`
and `PuzzlePack::from_json`, then unwrap.

Add:

```rust
fn puzzle_session(index: usize) -> SessionCore
```

Behavior: call `SessionInit::puzzle` with a dummy path,
`mini_pack()`, and `index`, then build `SessionCore::new(init)`.

### 2.3 Tests — group A

Add exactly these four tests:

* `puzzle_session_roots_at_puzzle_position_with_attacker_to_move`
* `puzzle_init_rejects_out_of_range_index`
* `puzzle_position_supports_full_study_flow`
* `new_resets_to_the_initially_loaded_puzzle`

### 2.4 Gate

Run `cargo test -p cli --test session`.

---

## Step 3 — Project gates

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Self-verification checklist

| File | Item | Contract |
|------|------|----------|
| `src/session.rs` | `#[allow(unused_imports)]` puzzle import | present; `Puzzle` unused until 07c |
| `src/session.rs` | `SessionInit::puzzle` field | `Option<PuzzleStudy>`, present in every literal |
| `src/session.rs` | `PuzzleStudy` | has `pack_path`, `index`, `pack` |
| `src/session.rs` | `SessionInit::puzzle` | validates index, sets opening from puzzle board, `swap2: false`, both seats human |
| `src/session.rs` | `SessionCore::puzzle` field | initialized from `init.puzzle` |
| `src/session.rs` | `SessionCore::puzzle()` | returns `Option<&PuzzleStudy>` |
| `src/session.rs` | `reset()` | restores `puzzle` from `init.puzzle` |
| `src/main.rs` | `play_init` | includes `puzzle: None` |
| `src/main.rs` | `selfplay_init` | includes `puzzle: None` |
| `tests/actor.rs` | struct literals | all include `puzzle: None` |
| `tests/study.rs` | struct literal | includes `puzzle: None` |
| `tests/session.rs` | `mini_pack()` | parses the fixture |
| `tests/session.rs` | `puzzle_session(index)` | builds a core from the pack |
| `tests/session.rs` | group A tests | all four pass |
