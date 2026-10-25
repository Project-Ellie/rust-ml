# Sub-chapter 05c — Swap2 as a session phase: solution

> **Opt-in solution — read only after you have tried the sub-chapter, or
> if you have been stuck for more than twenty minutes.**
>
> This file quotes the code that sub-chapter c adds or changes, as
> annotated snippets with line numbers. Line numbers refer to the files
> after the sub-chapter is applied. Deletions are described in prose; raw
> diffs are not shown. Verified green: `cargo test -p cli`,
> `cargo clippy --all-targets -- -D warnings`, and
> `cargo fmt --all -- --check` all pass from `gomoku/`. The two scripted
> demo transcripts (sub-chapter a's checklist) are byte-identical to
> before.

Reading order: the vocabulary first (`command.rs`), then the replay-log
machine (`opening.rs`), then the session routing (`session.rs`), then
the line interface (`repl.rs`), then the test kit and the tests.

## src/command.rs — the extended vocabulary

`Play` becomes a struct variant with the optional color token, and
three ceremony commands join the enum. All variants carry doc comments
now; the enum itself states its role.

src/command.rs, lines 15–55:

```rust
/// One command of the shared session language.
///
/// Text parsing ([`parse`]) is a convenience layer; this enum is the
/// real interface to `SessionCore`. The same values drive the study
/// table, the line fallback, the demos, and the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Play a move at the cursor. During the Swap2 opening this places
    /// the next ceremony stone instead. `color` is the optional
    /// override token; it is legal only during the opening.
    Play { mv: Move, color: Option<Color> },
    /// Let the AI play the side to move.
    Step,
    /// Go back N plies in the variation tree. During the Swap2 opening
    /// this removes the last N placements instead; the session knows
    /// the phase, so the two meanings never collide.
    Back { n: u32 },
    /// Go forward N plies in the variation tree.
    Forward { n: u32 },
    /// Choose child variation N at a branch point.
    Var { index: usize },
    /// Make seat(s) human.
    Take { seat: SeatSel },
    /// Release seat(s) to the AI.
    Release { seat: SeatSel },
    /// Set per-seat simulation counts.
    SetSims { seat: SeatSel, n: u32 },
    /// End the current Swap2 placement round.
    Done,
    /// Choose a color at a Swap2 choice phase.
    ChooseColor { color: Color },
    /// Decline the first Swap2 choice and place two more stones.
    More,
    /// Reset the session to its initial state. A Swap2 session
    /// restarts the opening ceremony.
    New,
    /// Show the help text.
    Help,
    /// Quit the session.
    Quit,
}
```

`OpeningPhase` follows the enum (lines 57–78). It is the runtime mirror
of the engine's four typestate markers, and it lives with the command
vocabulary because it is the payload of `Event::OpeningPhaseChanged`.
It derives serde for the snapshot's wire format; `snake_case` puts
`"placing3"`, `"first_choice"`, `"placing2"`, and `"final_choice"` on
the wire.

src/command.rs, lines 65–76:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningPhase {
    /// Player A places three stones: two Black and one White.
    Placing3,
    /// Player B chooses a color or asks for two more stones.
    FirstChoice,
    /// Player B places one Black and one White stone.
    Placing2,
    /// Player A chooses a color.
    FinalChoice,
}
```

The four new events close the `Event` enum. `StoneRemoved` is the one
addition to the design paper's list of three: an undo without an event
would be a visible change with no feedback. `OpeningFinished` carries
only the chosen color, because the side to move after the ceremony is
always White; the doc comment says so where a reader will look for it.

src/command.rs, lines 139–168:

```rust
    /// The session was reset to its initial opening.
    Reset,
    /// A stone was placed during the Swap2 opening.
    StonePlaced {
        /// Where the stone was placed.
        mv: Move,
        /// The color placed, after the default order or the override
        /// token was applied.
        color: Color,
    },
    /// `back` removed the last Swap2 placement.
    StoneRemoved {
        /// The placement that was removed.
        mv: Move,
    },
    /// The Swap2 opening entered a new phase.
    OpeningPhaseChanged {
        /// The phase the opening entered.
        phase: OpeningPhase,
    },
    /// The Swap2 opening finished; the game starts from its result.
    ///
    /// The payload is the chosen color and nothing else. The side to
    /// move is not carried: the engine guarantees that White moves
    /// first after the ceremony, because White always has fewer stones.
    OpeningFinished {
        /// The color the choosing player took.
        chosen: Color,
    },
}
```

Their `Display` arms (lines 199–212) follow the existing one-line style:
`Placed h8 White`, `Removed h8`, `Swap2 phase: FirstChoice`, `Swap2
finished: Black chosen`.

The seven new errors (lines 239–252) give every wrong-phase command its
own typed variant with one true sentence each. Note what is not here:
a failed `done` reuses the engine's `PlayError::BadOpeningCounts`
through the existing `IllegalMove` variant, because the count rule
belongs to the engine.

src/command.rs, lines 238–251:

```rust
    #[error("no Swap2 opening is in progress")]
    NotInOpening,
    #[error("the Swap2 opening is not at a placement phase")]
    NotAtPlacementPhase,
    #[error("the Swap2 opening is not at a choice phase")]
    NotAtChoicePhase,
    #[error("'more' applies only at the first Swap2 choice")]
    NotAtFirstChoice,
    #[error("the Swap2 opening has no placement to remove")]
    OpeningLogEmpty,
    #[error("a color token applies only during the Swap2 opening")]
    ColorOverrideOutsideOpening,
    #[error("the Swap2 opening is still in progress; choose a color first")]
    OpeningInProgress,
```

### The parser

The `play` arm and the bare-square fallback both end in the new helper,
so the two forms stay the same command. The ceremony arms sit between
`play` and `step`.

src/command.rs, lines 281–310:

```rust
        "play" => {
            let token = parse_required_token(&mut tokens, "square")?;
            let mv = parse_move(&token)?;
            let color = parse_optional_color(&mut tokens)?;
            Ok(Command::Play { mv, color })
        }

        "done" => {
            ensure_no_more(&mut tokens, "done")?;
            Ok(Command::Done)
        }

        "black" => {
            ensure_no_more(&mut tokens, "black")?;
            Ok(Command::ChooseColor {
                color: Color::Black,
            })
        }

        "white" => {
            ensure_no_more(&mut tokens, "white")?;
            Ok(Command::ChooseColor {
                color: Color::White,
            })
        }

        "more" => {
            ensure_no_more(&mut tokens, "more")?;
            Ok(Command::More)
        }
```

The helper accepts the token everywhere and lets the session decide
where it applies. Its doc comment records the split: parsing is about
text, phase is about state.

src/command.rs, lines 368–391:

```rust
/// Parse the optional color token of `play`: `black` or `white`.
///
/// The token is an override for the Swap2 opening, and the session
/// rejects it outside the opening. The parser accepts it
/// unconditionally: where a command applies is the session's business,
/// not the parser's.
fn parse_optional_color<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Result<Option<Color>, CommandError> {
    let Some(token) = tokens.next() else {
        return Ok(None);
    };
    let color = match token {
        "black" => Color::Black,
        "white" => Color::White,
        other => {
            return Err(CommandError::Parse(format!(
                "play expects a color: black|white, got '{other}'"
            )));
        }
    };
    ensure_no_more(tokens, "play")?;
    Ok(Some(color))
}
```

The parser tests: `parse_play_with_color_token` (lines 542–571) covers
the token on both `play` forms, case-insensitivity, and the two error
shapes; `parse_swap2_commands` (lines 573–593) covers the four new
commands and their extra-argument errors. The two pre-existing `play`
tests assert the struct variant with `color: None`.

## src/opening.rs (new, 356 lines)

The module doc (lines 1–18) states the lesson in one paragraph: the
log is the source of truth; the typestate is a validated projection of
the truth. `OpeningState` is two fields, and the doc comment explains
why the phase is not derivable from the log alone.

src/opening.rs, lines 1–24:

```rust
//! The session-side Swap2 machine: a replay log with the engine's
//! typestate as its validated projection.
//!
//! The engine's `Swap2<State>` typestate makes invalid transitions
//! impossible at compile time, but every transition consumes `self`.
//! A typestate value therefore cannot undo, cannot report its stones,
//! and cannot be stored across commands of different phases. This
//! module supplies the missing properties at runtime: [`OpeningState`]
//! keeps a replay log of `(Move, Color)` placements as the source of
//! truth and rebuilds the typestate from the log whenever the engine
//! must judge the position. The log is the truth; the typestate is a
//! validated projection of the truth.
//!
//! The rule split: the Swap2 rule fixes only the final stone counts of
//! each placement round (two Black and one White, then one more Black
//! and one more White after `more`). The placement order is free. The
//! default order alternates, black first, within each round; a color
//! token on `play` overrides the next stone's color.

use engine::{Board, Color, Move, Swap2};
use engine::{FinalChoice, FirstChoice, Placing2, Placing3};
use serde::{Deserialize, Serialize};

use crate::command::{CommandError, Event, OpeningPhase};
```

src/opening.rs, lines 26–37:

```rust
/// The session-side record of a Swap2 ceremony in progress.
///
/// Two fields carry the whole state: the runtime phase (which mirrors
/// the engine's typestate marker) and the replay log of placements.
/// The phase is not derivable from the log alone: three placements can
/// mean "round complete, `done` accepted" (FirstChoice) or "third
/// stone placed, `done` not yet given" (Placing3).
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningState {
    phase: OpeningPhase,
    log: Vec<(Move, Color)>,
}
```

`OpeningSnapshot` (lines 39–60) is the read model a front-end draws
from. Its three fields are the whole ceremony: phase, placements in
order, and the color the next `play` will use.

### The default color

The one rule with a sharp edge: the alternation restarts per round. The
fourth stone overall is the first stone of round two and defaults to
black. A global parity rule is wrong at exactly that stone; the
in-module test `the_default_color_alternates_per_round_starting_with_black`
(lines 305–322) pins the restart.

src/opening.rs, lines 77–99:

```rust
    /// The color the next `play` uses when no color token overrides it.
    ///
    /// The default alternates within each round, black first. The
    /// count restarts per round: the fourth stone overall is the first
    /// stone of the second round, so it defaults to black again. At a
    /// choice phase there is no next placement, so the answer is
    /// `None`.
    #[must_use]
    pub fn default_next_color(&self) -> Option<Color> {
        let round_start = match self.phase {
            OpeningPhase::Placing3 => 0,
            OpeningPhase::Placing2 => 3,
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => return None,
        };
        let index_in_round = self.log.len() - round_start;
        Some(if index_in_round.is_multiple_of(2) {
            Color::Black
        } else {
            Color::White
        })
    }

    /// Place the next stone of the ceremony.
```

### place, undo, done

`place` routes the color default, asks the engine, and grows the log
only after acceptance. The `None` arm is what rejects `play` at a
choice phase without touching the engine.

src/opening.rs, lines 99–122:

```rust
    /// Place the next stone of the ceremony.
    ///
    /// The engine remains the rules authority: the typestate is
    /// rebuilt from the log, and its `place` rejects an occupied
    /// square. The log grows only after the engine accepts the
    /// placement, so the log can never hold a stone the engine
    /// refused.
    pub fn place(&mut self, mv: Move, color: Option<Color>) -> Result<Vec<Event>, CommandError> {
        // A choice phase has no next stone; the `None` default makes
        // `play` there a typed error without touching the engine.
        let color = match color.or_else(|| self.default_next_color()) {
            Some(color) => color,
            None => return Err(CommandError::NotAtPlacementPhase),
        };
        match self.phase {
            OpeningPhase::Placing3 => rebuild_placing3(&self.log).place(mv, color)?,
            OpeningPhase::Placing2 => rebuild_placing2(&self.log).place(mv, color)?,
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => {
                return Err(CommandError::NotAtPlacementPhase);
            }
        }
        self.log.push((mv, color));
        Ok(vec![Event::StonePlaced { mv, color }])
    }
```

`undo` is the method the whole design exists for, and it contains no
engine call. The comment explains why that is safe: every prefix of
the log was once the live state. The phase after the removal is a
function of the log length and never a choice phase.

src/opening.rs, lines 124–158:

```rust
    /// Take back the last placement(s) — the meaning of `back` during
    /// the ceremony.
    ///
    /// No engine rebuild is needed here. The log only grows through
    /// validated placements, so every prefix of the log was once the
    /// live state and is valid by construction; removing entries cannot
    /// produce an invalid state. The phase after the removal is a
    /// function of the log length, and it is never a choice phase:
    /// removing the stone that completed a round takes the `done` back
    /// with it, and reaching a choice again takes a fresh `done`.
    pub fn undo(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        if self.log.is_empty() {
            return Err(CommandError::OpeningLogEmpty);
        }
        let mut events = Vec::new();
        // `back N` removes up to N placements; the count clamps at the
        // log length the way tree navigation clamps at the root.
        let removals = (n as usize).min(self.log.len());
        for _ in 0..removals {
            let (mv, _) = self.log.pop().expect("the length was checked above");
            events.push(Event::StoneRemoved { mv });
        }
        // Four or five stones can only be a second round in progress;
        // three or fewer belong to the first round.
        let phase = if self.log.len() >= 4 {
            OpeningPhase::Placing2
        } else {
            OpeningPhase::Placing3
        };
        if phase != self.phase {
            self.phase = phase;
            events.push(Event::OpeningPhaseChanged { phase });
        }
        Ok(events)
    }
```

`done` and `more` (lines 160–203) run the engine transitions. `done`
returns the engine's `BadOpeningCounts` on rejection and leaves the
state untouched, so the round continues. `more` routes through
`place_two_more` although that transition cannot fail: every protocol
transition goes through the typestate, so the rules live in one place.

src/opening.rs, lines 160–183:

```rust
    /// End the current placement round.
    ///
    /// The engine validates the counts: the rebuild replays the log
    /// into the typestate, and `finish` accepts only two Black and one
    /// White in the first round or three Black and two White in the
    /// second. On rejection the phase and the log stay untouched, so
    /// the user can place or remove stones and try again.
    pub fn done(&mut self) -> Result<Vec<Event>, CommandError> {
        let next = match self.phase {
            OpeningPhase::Placing3 => {
                rebuild_placing3(&self.log).finish()?;
                OpeningPhase::FirstChoice
            }
            OpeningPhase::Placing2 => {
                rebuild_placing2(&self.log).finish()?;
                OpeningPhase::FinalChoice
            }
            OpeningPhase::FirstChoice | OpeningPhase::FinalChoice => {
                return Err(CommandError::NotAtPlacementPhase);
            }
        };
        self.phase = next;
        Ok(vec![Event::OpeningPhaseChanged { phase: next }])
    }
```

### choose and the rebuild helpers

`choose` takes `&self`: it reports the finished board and changes
nothing. The session owns the boundary — the board becomes the tree
root and the opening state is cleared there.

src/opening.rs, lines 204–232:

```rust
    /// Choose a color at a choice phase and report the finished board.
    ///
    /// The board comes from the engine's `take_black` / `take_white`,
    /// which guarantee White to move, because White always has fewer
    /// stones after the ceremony. The method is read-only: the session
    /// decides what the board becomes (the root of a fresh variation
    /// tree) and clears the opening state.
    pub fn choose(&self, color: Color) -> Result<Board, CommandError> {
        match self.phase {
            OpeningPhase::FirstChoice => {
                let state = rebuild_first_choice(&self.log);
                Ok(match color {
                    Color::Black => state.take_black(),
                    Color::White => state.take_white(),
                })
            }
            OpeningPhase::FinalChoice => {
                let state = rebuild_final_choice(&self.log);
                Ok(match color {
                    Color::Black => state.take_black(),
                    Color::White => state.take_white(),
                })
            }
            OpeningPhase::Placing3 | OpeningPhase::Placing2 => Err(CommandError::NotAtChoicePhase),
        }
    }
}

impl Default for OpeningState {
```

The four rebuild helpers (lines 238–295) are the projection. Each
replays the log into the typestate; the `expect` and `debug_assert`
messages document invariants rather than handle errors. Only
`rebuild_placing2` and `rebuild_final_choice` are quoted here.

src/opening.rs, lines 253–270:

```rust
/// Replay the log into `Swap2<Placing2>`: the first three entries are
/// the finished first round; the rest are second-round placements.
fn rebuild_placing2(log: &[(Move, Color)]) -> Swap2<Placing2> {
    debug_assert!(
        log.len() >= 3,
        "the second round starts after three placements"
    );
    let mut placing2 = rebuild_placing3(&log[..3])
        .finish()
        .expect("the first round was count-validated by `done`")
        .place_two_more();
    for &(mv, color) in &log[3..] {
        placing2
            .place(mv, color)
            .expect("the log contains only engine-validated placements");
    }
    placing2
}
```

The in-module tests (lines 297–356) pin the alternation restart, the
undo-to-phase rule, and the snapshot mirror.

## src/session.rs

### Construction and state

`SessionInit` gains the flag (line 37: `pub swap2: bool`), and every
existing constructor sets it to `false`. The builder is chainable.

src/session.rs, lines 83–94:

```rust
    /// Enable or disable the Swap2 opening ceremony.
    ///
    /// A ceremony requires an empty `opening` board: ceremony stone
    /// counts can differ by two mid-round, which no valid game board
    /// can hold. [`SessionCore::new`] asserts the combination, because
    /// mixing a ceremony with a preset position is a programming
    /// error, not user input.
    #[must_use]
    pub fn with_swap2(mut self, swap2: bool) -> Self {
        self.swap2 = swap2;
        self
    }
```

`SessionCore` gains the `opening: Option<OpeningState>` field (line
236) with a comment that states the tree's role during the ceremony:
it still holds the empty initial board, because ceremony placements are
not game moves. The construction assert and its reasoning:

src/session.rs, lines 240–254:

```rust
    /// Create a new session from the given initial state.
    ///
    /// # Panics
    /// Panics when `init.swap2` is set on a non-empty opening board.
    /// That combination is a programming error, not user input, so it
    /// is an assert rather than a typed error.
    pub fn new(init: SessionInit) -> Self {
        // A Swap2 ceremony is not a game position: its stone counts can
        // differ by two mid-round, which no valid `Board` can hold. A
        // ceremony on top of a preset opening would mix two sources of
        // truth, so the combination is rejected at construction.
        assert!(
            !init.swap2 || init.opening == Board::new(),
            "swap2 requires an empty opening board"
        );
```

### The snapshot and the wire format

`Snapshot` gains `pub opening: Option<OpeningSnapshot>` (line 152), and
`snapshot()` maps the machine into the read model (line 288:
`opening: self.opening.as_ref().map(OpeningState::snapshot)`). The
serde impl writes the field only when it is `Some`, which keeps the
wire format of an ordinary session byte-identical to before; the
`Deserialize` helper gains the field as an `Option`, which serde treats
as implicitly optional, so old snapshots still parse.

src/session.rs, lines 157–173:

```rust
        use serde::ser::SerializeStruct;
        // The `opening` field is written only while a ceremony is in
        // progress, so the wire format of an ordinary session is
        // byte-identical to before the Swap2 sub-chapter.
        let field_count = if self.opening.is_some() { 7 } else { 6 };
        let mut state = serializer.serialize_struct("Snapshot", field_count)?;
        state.serialize_field("ply", &self.ply)?;
        state.serialize_field("branch_options", &self.branch_options)?;
        state.serialize_field("black", &self.black)?;
        state.serialize_field("white", &self.white)?;
        state.serialize_field("status", &status_as_str(self.status))?;
        state.serialize_field("last_move", &self.last_move)?;
        if let Some(opening) = &self.opening {
            state.serialize_field("opening", opening)?;
        }
        state.end()
    }
```

`board()` gains the doc comment that records where the stones live
during the ceremony (lines 292–301).

### Phase routing

`execute` gains three arms, and `play`/`back` route by phase. The
routing comments explain the two dual meanings.

src/session.rs, lines 336–353:

```rust
    pub fn execute(&mut self, cmd: Command) -> Result<Vec<Event>, CommandError> {
        match cmd {
            Command::Play { mv, color } => self.execute_play(mv, color),
            Command::Back { n } => self.execute_back(n),
            Command::Forward { n } => self.execute_forward(n),
            Command::Var { index } => self.execute_var(index),
            Command::New => self.execute_new(),
            Command::Take { seat } => self.execute_take(seat),
            Command::Step => self.execute_step(),
            Command::Release { seat } => self.execute_release(seat),
            Command::SetSims { seat, n } => self.execute_set_sims(seat, n),
            Command::Done => self.execute_done(),
            Command::ChooseColor { color } => self.execute_choose_color(color),
            Command::More => self.execute_more(),

            Command::Help | Command::Quit => Ok(vec![]),
        }
    }
```

`execute_done` and `execute_more` (lines 355–369) are one-liners into
the machine, with `NotInOpening` outside the ceremony. The boundary
method:

src/session.rs, lines 371–382:

```rust
    /// Choosing a color ends the ceremony: the engine hands back the
    /// finished opening board, which becomes the root of a fresh
    /// variation tree, and the normal command set takes over.
    fn execute_choose_color(&mut self, color: Color) -> Result<Vec<Event>, CommandError> {
        let Some(opening) = &self.opening else {
            return Err(CommandError::NotInOpening);
        };
        let board = opening.choose(color)?;
        self.history = History::from_opening(&board);
        self.opening = None;
        Ok(vec![Event::OpeningFinished { chosen: color }])
    }
```

`execute_play` and `execute_back` route the dual-meaning commands.

src/session.rs, lines 516–529 and 558–563:

```rust
    pub fn execute_play(
        &mut self,
        mv: Move,
        color: Option<Color>,
    ) -> Result<Vec<Event>, CommandError> {
        // Phase routing: during the ceremony `play` places a stone (the
        // color token is the override); afterwards `play` is a tree
        // move and the token is a typed error.
        if let Some(opening) = &mut self.opening {
            return opening.place(mv, color);
        }
        if color.is_some() {
            return Err(CommandError::ColorOverrideOutsideOpening);
        }
```

```rust
    pub fn execute_back(&mut self, n: u32) -> Result<Vec<Event>, CommandError> {
        // `back` is phase-dependent: it removes placements during the
        // ceremony and navigates the variation tree afterwards.
        if let Some(opening) = &mut self.opening {
            return opening.undo(n);
        }
```

`execute_step` refuses during the ceremony (lines 385–392), and `reset`
restores the ceremony for Swap2 sessions (lines 652–659).

## src/repl.rs

The help text gains a five-line Swap2 block (lines 38–44).
`print_state` inserts one line during the ceremony (lines 133–137),
produced by `opening_line` (lines 154–178):

```rust
/// The one state line of a ceremony in progress: phase, placements in
/// order, and the color the next `play` defaults to.
///
/// At a choice phase there is no next stone, so the `next` part is
/// left out. The line appears only in Swap2 sessions; ordinary
/// transcripts are byte-identical to before.
fn opening_line(opening: &OpeningSnapshot) -> String {
    let stones = if opening.placements.is_empty() {
        "(none)".to_string()
    } else {
        opening
            .placements
            .iter()
            .map(|(mv, color)| format!("{}:{color:?}", algebraic(*mv)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    match opening.default_next_color {
        Some(color) => format!(
            "swap2 {:?}  stones: {stones}  next: {color:?}",
            opening.phase
        ),
        None => format!("swap2 {:?}  stones: {stones}", opening.phase),
    }
}
```

Two in-module tests (lines 219–282) pin both sides of the contract:
ordinary sessions print no swap2 line, and a ceremony prints the line
with the override color and without `next` at the right times.

## The test kit and the tests

`tests/common/mod.rs`: `play` builds the struct variant with
`color: None`; `play_color` (line 43) passes the token; `swap2_core`
(line 52) builds a hotseat ceremony session; `event_type` learns the
four new names (lines 79–82).

tests/opening.rs, lines 1–40:

```rust
//! Sub-chapter 05c integration tests: the Swap2 opening as a session
//! phase, driven through the public command language.
//!
//! Every test is headless: commands in, events and snapshots out. The
//! engine's typestate stays the rules authority; these tests assert the
//! session-level behavior around it — the replay log, the phase
//! routing, the typed errors, and the handover to the variation tree.

use cli::command::{Command, CommandError, Event, OpeningPhase};
use cli::session::{SessionCore, SessionInit, Snapshot};
use engine::{Board, Color, PlayError};

mod common;
use common::{event_types, mv, play, play_color, swap2_core};

/// Place the first round with the default alternation (black, white,
/// black) and end it with `done`. Returns the events of `done`.
fn place_first_round(core: &mut SessionCore) -> Vec<Event> {
    play(core, 7, 7);
    play(core, 8, 8);
    play(core, 9, 9);
    core.execute(Command::Done).unwrap()
}

/// Run the second round: `more`, two placements, `done`.
fn place_second_round(core: &mut SessionCore) {
    core.execute(Command::More).unwrap();
    play(core, 5, 5);
    play(core, 4, 4);
    core.execute(Command::Done).unwrap();
}

/// Count the stones of `color` through the public per-cell API.
/// (`Board::stones` is engine-internal; a test goes cell by cell.)
fn stone_count(board: &Board, color: Color) -> usize {
    (0..15)
        .flat_map(|r| (0..15).map(move |c| mv(r, c)))
        .filter(|&m| board.stone_at(m) == Some(color))
        .count()
}
```

`tests/opening.rs` (new, 536 lines, 20 tests) drives everything through
the public API. Two local helpers carry the protocol: `place_first_round`
plays the default order and ends the round with `done`;
`place_second_round` plays the `more` path. One test as an example of
the style:

```rust
#[test]
fn back_on_an_empty_log_is_a_typed_error() {
    let mut core = swap2_core();
    let err = core.execute(Command::Back { n: 1 }).unwrap_err();
    assert_eq!(err, CommandError::OpeningLogEmpty);
}
```

The white-to-move invariant is asserted at the end of every happy path:
after `OpeningFinished` the board's `to_move` is White, and the first
tree move lands as a white stone. `stone_count` in the test file counts
stones through the public `stone_at`, because `Board::stones` is
engine-internal.

The remaining files change mechanically: `src/tui/render.rs` and the
existing test files update their `Command::Play` construction sites,
and the `Snapshot` literals in `tests/session.rs` gain `opening: None`
(the wire-format test's expected JSON is unchanged, which is the
byte-stability proof).

## Verification

From `gomoku/`, after the sub-chapter:

* `cargo test -p cli` — 101 tests pass (74 before): 47 lib tests (40
  before; +3 opening unit tests, +2 parser tests, +2 repl tests), the
  new `tests/opening.rs` with 20 tests, and the unchanged suites (7
  actor, 9 session, 13 history, 4 study, 1 doctest).
* `cargo clippy --all-targets -- -D warnings` — clean.
* `cargo fmt --all -- --check` — clean.
* Both demo transcripts are byte-identical to before.

A note on size: `session.rs` grew to 951 lines, past the chapter's
budget, and `command.rs` to 702. The budget's own rule applies: a file
past the budget holds more than one concern. The ceremony logic itself
moved to `opening.rs` (356 lines, most of them comments the chapter
standard requires); what remains in `session.rs` is routing and
construction. A future split of `session.rs` (snapshot serde versus
command execution) is visible but is not this sub-chapter.
