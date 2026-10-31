# Sub-chapter 07b — The puzzle enrolls: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

The session layer imports only the puzzle types it needs at this slice.
`Puzzle` is not imported because it is unused until sub-chapter 07c; the
import widens then.

`gomoku/crates/cli/src/session.rs`, line 12:

```rust
use crate::puzzle::{PuzzleError, PuzzlePack};
```

`PathBuf` is also needed for the pack path stored in `PuzzleStudy`.

`gomoku/crates/cli/src/session.rs`, line 18:

```rust
use std::path::PathBuf;
```

`SessionInit` grows the `puzzle` field. Every constructor and every
struct literal in the crate must include it.

`gomoku/crates/cli/src/session.rs`, lines 38–39:

```rust
    /// Puzzle study context when the session was started from a pack.
    pub puzzle: Option<PuzzleStudy>,
```

`PuzzleStudy` carries the pack path, the current index, and the whole
parsed pack. Keeping the pack in memory lets later navigation switch
puzzles without file I/O.

`gomoku/crates/cli/src/session.rs`, lines 46–54:

```rust
/// Puzzle study context carried by a session started from a pack.
///
/// The whole pack stays in memory so `next` / `prev` can load adjacent
/// puzzles without further I/O.
#[derive(Clone, Debug, PartialEq)]
pub struct PuzzleStudy {
    /// Path the pack was loaded from (for display and error messages).
    pub pack_path: PathBuf,
    /// Zero-based index of the currently loaded puzzle.
    pub index: usize,
    /// The parsed pack.
    pub pack: PuzzlePack,
}
```

`SessionInit::puzzle` validates the index, clones the puzzle board as
the opening, and builds a hotseat session with Swap2 disabled.

`gomoku/crates/cli/src/session.rs`, lines 142–168:

```rust
    /// Hotseat study session seeded from puzzle `index` of `pack`.
    ///
    /// The puzzle position becomes the root of the variation tree with
    /// the attacker to move; both seats start human. Seats and AI
    /// configuration can be adjusted afterwards with struct-update
    /// syntax, exactly as for the other constructors.
    ///
    /// # Errors
    /// Returns `PuzzleError::IndexOutOfRange` if `index` is not in the pack.
    pub fn puzzle(pack_path: PathBuf, pack: PuzzlePack, index: usize) -> Result<Self, PuzzleError> {
        let opening = pack.get(index)?.board.clone();
        Ok(Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening,
            swap2: false,
            puzzle: Some(PuzzleStudy {
                pack_path,
                index,
                pack,
            }),
        })
    }
```

The existing constructors set `puzzle: None`. The hotseat constructor
is shown below; the other three receive the same field.

`gomoku/crates/cli/src/session.rs`, lines 71–83:

```rust
    pub fn hotseat() -> Self {
        Self {
            black: Occupant::Human,
            white: Occupant::Human,
            default_config: AiConfig::default(),
            black_sims: None,
            white_sims: None,
            seed: None,
            opening: Board::new(),
            swap2: false,
            puzzle: None,
        }
    }
```

`SessionCore` stores its own copy of the puzzle context so the public
accessor can return it without exposing `init`.

`gomoku/crates/cli/src/session.rs`, lines 272–273:

```rust
    /// Puzzle study context; `Some` only for sessions started from a pack.
    puzzle: Option<PuzzleStudy>,
```

`SessionCore::new` copies the context from `init.puzzle`.

`gomoku/crates/cli/src/session.rs`, lines 303–304:

```rust
            puzzle: init.puzzle.clone(),
            init,
```

The accessor returns the active `PuzzleStudy` by reference.

`gomoku/crates/cli/src/session.rs`, lines 337–341:

```rust
    /// The puzzle study context, if this session was started from a pack.
    #[must_use]
    pub fn puzzle(&self) -> Option<&PuzzleStudy> {
        self.puzzle.as_ref()
    }
```

`reset` restores the puzzle context from `init.puzzle`, so `new`
returns to the initially loaded puzzle.

`gomoku/crates/cli/src/session.rs`, line 671:

```rust
        self.puzzle = self.init.puzzle.clone();
```

The `puzzle: None` ripple reaches the binary constructors. `play_init`
and `selfplay_init` each add the field to their `SessionInit` literals.

`gomoku/crates/cli/src/main.rs`, lines 258–269:

```rust
    SessionInit {
        black,
        white,
        default_config: base,
        black_sims: search.black_sims,
        white_sims: search.white_sims,
        seed: search.seed,
        opening: Board::new(),
        swap2,
        puzzle: None,
    }
```

`gomoku/crates/cli/src/main.rs`, lines 276–286:

```rust
    SessionInit {
        black: Occupant::Ai(seat_config(&base, Color::Black, search)),
        white: Occupant::Ai(seat_config(&base, Color::White, search)),
        default_config: base,
        black_sims: search.black_sims,
        white_sims: search.white_sims,
        seed: search.seed,
        opening: Board::new(),
        swap2: false,
        puzzle: None,
    }
```

The ripple also touches the explicit `SessionInit` literals in the
test suite. `tests/actor.rs` has three of them.

`gomoku/crates/cli/tests/actor.rs`, lines 32–42:

```rust
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(ai_config_tactics()),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
        puzzle: None,
    });
```

`gomoku/crates/cli/tests/actor.rs`, lines 94–107:

```rust
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Ai(AiConfig {
            simulations: 300,
            ..ai_config_tactics()
        }),
        white: Occupant::Human,
        default_config: ai_config_tactics(),
        black_sims: None,
        white_sims: None,
        seed: Some(1),
        opening: board,
        swap2: false,
        puzzle: None,
    });
```

`gomoku/crates/cli/tests/actor.rs`, lines 148–161:

```rust
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: AiConfig {
            simulations: 10, // should be overwritten on both seats
            ..ai_config_tactics()
        },
        black_sims: Some(400),
        white_sims: Some(50),
        seed: Some(1),
        opening: Board::new(),
        swap2: false,
        puzzle: None,
    });
```

`tests/study.rs` has one explicit literal.

`gomoku/crates/cli/tests/study.rs`, lines 156–166:

```rust
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Human,
        default_config: default_ai_config(),
        black_sims: Some(50),
        white_sims: Some(400),
        seed: Some(1),
        opening: engine::Board::new(),
        swap2: false,
        puzzle: None,
    });
```

The new integration tests live in `tests/session.rs`. They import the
puzzle module and define two helpers.

`gomoku/crates/cli/tests/session.rs`, line 7:

```rust
use cli::puzzle;
```

`gomoku/crates/cli/tests/session.rs`, lines 317–329:

```rust
fn mini_pack() -> puzzle::PuzzlePack {
    puzzle::PuzzlePack::from_json(include_str!("fixtures/mini-pack.json")).unwrap()
}

fn puzzle_session(index: usize) -> SessionCore {
    let init = SessionInit::puzzle(
        std::path::PathBuf::from("mini-pack.json"),
        mini_pack(),
        index,
    )
    .unwrap();
    SessionCore::new(init)
}
```

The first test checks that a valid index loads the right board and
attacker, and that the puzzle context reports the index and pack size.

`gomoku/crates/cli/tests/session.rs`, lines 331–349:

```rust
#[test]
fn puzzle_session_roots_at_puzzle_position_with_attacker_to_move() {
    let core = puzzle_session(1);
    assert_eq!(core.board(), mini_pack().items[1].board);
    assert_eq!(core.board().to_move(), Color::White);
    let study = core.puzzle().expect("puzzle context");
    assert_eq!(study.index, 1);
    assert_eq!(study.pack.items.len(), 3);
}

#[test]
fn puzzle_init_rejects_out_of_range_index() {
    let err = SessionInit::puzzle(std::path::PathBuf::from("mini-pack.json"), mini_pack(), 3)
        .unwrap_err();
    assert!(matches!(
        err,
        puzzle::PuzzleError::IndexOutOfRange { index: 3, count: 3 }
    ));
}
```

The next test proves that the puzzle root behaves like any other study
position: moves, back, branching, forward, branch points, and
variation selection all emit the expected events.

`gomoku/crates/cli/tests/session.rs`, lines 351–377:

```rust
#[test]
fn puzzle_position_supports_full_study_flow() {
    let mut core = puzzle_session(2);
    // Play the claimed line head, then branch with a wrong try.
    play(&mut core, 7, 10); // k8 — the claimed first move
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = play(&mut core, 10, 10);
    assert!(
        events.iter().any(|e| matches!(e, &Event::Branched { .. })),
        "expected Branched, got {events:?}"
    );
    core.execute(Command::Back { n: 1 }).unwrap();
    let events = core.execute(Command::Forward { n: 1 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::BranchPoint { .. })),
        "expected BranchPoint, got {events:?}"
    );
    let events = core.execute(Command::Var { index: 0 }).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, &Event::CursorMoved { .. })),
        "expected CursorMoved, got {events:?}"
    );
}
```

The last test dirties the tree with a move and asserts that `new`
returns to the initially loaded puzzle root. At this slice the session
cannot navigate to other puzzles yet, so the reset target is simply
the original index.

`gomoku/crates/cli/tests/session.rs`, lines 379–388:

```rust
#[test]
fn new_resets_to_the_initially_loaded_puzzle() {
    let mut core = puzzle_session(0);
    // Dirty the tree; reset must return to the loaded puzzle root.
    play(&mut core, 9, 9);
    let events = core.execute(Command::New).unwrap();
    assert!(events.iter().any(|e| matches!(e, &Event::Reset)));
    assert_eq!(core.puzzle().unwrap().index, 0);
    assert_eq!(core.board(), mini_pack().items[0].board);
}
```
