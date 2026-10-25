# Sub-chapter 05c — Swap2 as a session phase

Status: IMPLEMENTED. This sub-chapter makes the opening ceremony part
of the session state. It adds commands to the shared language; it adds
one module; it changes no behavior of sessions without Swap2.

## Abstract

The engine has a complete Swap2 machine, built as a typestate:
`Swap2<Placing3>` becomes `Swap2<FirstChoice>` when the counts are
right, and an invalid transition is a compile error. But a typestate
transition consumes `self`, so the machine cannot undo, cannot report
its stones, and cannot survive across commands of different phases in
one variable. The reference build answered this with a 440-line
re-implementation of the protocol inside the TUI. This sub-chapter
shows the other answer: keep a replay log of placements as the source
of truth, and rebuild the engine's typestate from the log whenever a
rule must be checked. The session gains the ceremony as a phase with
six commands in the same language as everything else: `play h8
[color]`, `back`, `done`, `black`, `white`, `more`.

## Glossary

| Term | Definition |
|------|------------|
| **Ceremony** | The Swap2 opening protocol as the session runs it: two placement rounds and two choice phases. |
| **Typestate** | The engine's `Swap2<State>` type, where the state is a type parameter. Invalid transitions do not compile. |
| **Replay log** | The `Vec<(Move, Color)>` the session keeps. Every placement that the engine accepted, in order. |
| **Validated projection** | A typestate value rebuilt from the log. It is computed on demand and discarded; the log stays the truth. |
| **Phase** | The runtime mirror of the typestate marker: `OpeningPhase::Placing3`, `FirstChoice`, `Placing2`, `FinalChoice`. |
| **Placement round** | One placing phase: three stones in round one, two stones in round two. The rule fixes the counts per round; the order is free. |
| **Color token** | The optional `black`/`white` argument of `play`, which overrides the default color of the next stone. |
| **Phase routing** | The meaning of a command depends on the session phase. `back` removes a placement during the ceremony and navigates the tree afterwards. |

## Context

Read [the design paper](../00-design.md) first. Its section
"c — Swap2 as a session phase" assigns this sub-chapter, and its header
records the decided Swap2 vocabulary and the free placement order.
Read [`gomoku/crates/engine/src/opening.rs`](../../../../gomoku/crates/engine/src/opening.rs)
next. It is the machine you will reuse, not re-implement. The engine
chapter ([docs/13-engine-design.md](../../../../docs/13-engine-design.md))
explains why the board stores absolute colors: a Swap2 position cannot
be expressed in a relative me/you store.

Two properties of the engine API shape everything below:

1. **Transitions consume `self`.** `finish`, `place_two_more`,
   `take_black`, and `take_white` take ownership. A stored typestate
   value can move forward once and is then gone. Undo is impossible.
2. **The typestate has no read access.** `Swap2` exposes no stones.
   Only `take_black`/`take_white` produce something readable: a
   `Board`. And a mid-ceremony position is not always a valid `Board`:
   two Black stones and zero White stones break the count invariant of
   `Board::from_position`.

So the session cannot hold the typestate. It holds data instead.

## Intention

1. Extend the command language in `src/command.rs`: `play` gains the
   optional color token; `done`, `black`, `white`, and `more` are new
   commands; `OpeningPhase` joins the vocabulary as an event payload.
2. Create `src/opening.rs`: `OpeningState` keeps the phase and the
   replay log; every rule decision goes through a rebuilt typestate.
3. Extend `SessionCore`: `SessionInit` gains `swap2`, the core gains
   the opening state, and `execute` routes the dual-meaning commands
   (`play`, `back`) by phase.
4. Expose the ceremony in `Snapshot`, so the renderer and the REST
   seam can draw it without knowing the rules.
5. Cover the ceremony headless: the full protocol paths, the undo
   rules, the count validation, and the typed errors.
6. Prove that nothing else changed: the two scripted demo transcripts
   stay byte-identical.

## Mental mapping

### The lesson: the log is the source of truth

The typestate and the session solve different halves of one problem.
The typestate gives compile-time proof that no invalid transition
exists in the code. The session needs runtime behavior the typestate
cannot have: undo, inspection, and persistence across commands. The
combination gives both, through one rule:

**The log is the truth. The typestate is a validated projection of the
truth.**

Every placement enters the log only after the engine accepted it.
Every rule check (`done` counts, occupied squares, the final board)
rebuilds the typestate from the log and asks the engine. The session
never re-implements a Swap2 rule. The rebuild costs at most five
`place` calls, so the price of keeping the rules in exactly one place
is small.

The reference build chose the opposite: it mirrored the protocol in
440 lines of TUI code, with its own counts and its own transitions.
Two machines, one protocol, and the copy can drift. Here the copy
cannot drift, because there is no copy.

### Why undo needs no rebuild

`back` pops entries from the log. No engine call is involved, and none
is needed: the log only ever grows through validated placements, so
every prefix of the log was once the live state. A prefix of a valid
sequence is a valid state. Undo cannot break anything.

The phase after an undo is a function of the log length: three or
fewer stones means Placing3, four or five means Placing2. An undo never
lands in a choice phase. Removing the stone that completed a round
takes the `done` back with it; reaching the choice again takes a fresh
`done`. One rule covers both rounds, and the user can always see why:
the stones on the board are the state.

### Why the ceremony has no `Board`

`Board::from_position` requires the stone counts to differ by at most
one. Mid-ceremony positions can differ by two (the default order opens
black, black). No valid `Board` can hold such a position, and the
engine is right to reject it: it is not a game position.

Consequence: during the ceremony, `SessionCore::board()` returns the
empty initial board, and the stones live in `Snapshot.opening` as
`(Move, Color)` pairs. The renderer draws them from there. Only the
finished opening — produced by the engine's `take_black`/`take_white`,
which guarantee a legal position with White to move — becomes a
`Board` and the root of a fresh variation tree.

### Phase routing: one language, two meanings

The ceremony adds no second interpreter. The same parser produces the
same `Command` values, and `SessionCore::execute` routes them by
phase:

- `play` places a ceremony stone while the opening is in progress and
  a tree move afterwards. The color token is legal only during the
  ceremony; outside it, the token is a typed error.
- `back` removes placements while the opening is in progress and
  navigates the tree afterwards.
- `done`, `black`, `white`, and `more` exist only during the ceremony.
  Outside it, each returns the typed `NotInOpening` error.

The parser accepts the color token everywhere and the session decides
where it applies. That split is deliberate: parsing is about text,
phase is about state. A parse error means "I cannot read this"; a
phase error means "this does not apply now".

### Errors are phase reports

Every wrong-phase command returns its own typed error, and each error
message says one true sentence. `more` outside the first choice says
`'more' applies only at the first Swap2 choice`, which holds at every
other phase. `black` during a placement round says the opening `is not
at a choice phase`. Tests match the variants, so the messages and the
routing cannot drift apart.

Two errors deserve a note:

- A failed `done` returns `IllegalMove(PlayError::BadOpeningCounts)`.
  The count rule belongs to the engine, so its error is the engine's
  own `PlayError`, not a new session variant. The phase and the log
  stay untouched; the user places or removes stones and tries again.
- `step` during the ceremony returns `OpeningInProgress`. The AI
  searches game positions, and a ceremony is not one. A move searched
  during the ceremony would be discarded when the ceremony finishes
  and resets the tree, so the session refuses instead.

### Reset returns to the start

`new` resets the session to its initial phase. A session that started
in the ceremony resets into the ceremony: empty log, Placing3. The
choice is recorded here because the alternative (reset to the empty
tree, losing Swap2) would silently change what the session is.

## Low-level design

### `src/command.rs`

```rust
pub enum Command {
    Play { mv: Move, color: Option<Color> },
    // ... existing variants unchanged ...
    Done,
    ChooseColor { color: Color },
    More,
}

pub enum OpeningPhase { Placing3, FirstChoice, Placing2, FinalChoice }
```

`Play` becomes a struct variant. Every construction site changes; the
shared test kit's `play` helper absorbs most of them. The parser
accepts the color token on the explicit `play h8 white` form and on
the bare-square form `h8 white`, because the two forms are documented
as the same command.

`OpeningPhase` lives in the vocabulary module because it is the
payload of `Event::OpeningPhaseChanged`, the way `SeatSel` is the
payload of seat commands. The four variants mirror the engine's four
typestate markers one to one.

New events: `StonePlaced { mv, color }`, `StoneRemoved { mv }`,
`OpeningPhaseChanged { phase }`, and `OpeningFinished { chosen }`. The
design paper listed three; `StoneRemoved` is the fourth. Without it an
undo produces no feedback, and the events panel would show nothing for
a visible change. `OpeningFinished` carries only the chosen color: the
side to move after the ceremony is always White, so carrying it would
repeat what the engine already guarantees.

New errors: `NotInOpening`, `NotAtPlacementPhase`, `NotAtChoicePhase`,
`NotAtFirstChoice`, `OpeningLogEmpty`, `ColorOverrideOutsideOpening`,
and `OpeningInProgress`.

### `src/opening.rs` (new)

```rust
pub struct OpeningState { phase: OpeningPhase, log: Vec<(Move, Color)> }

impl OpeningState {
    pub fn new() -> Self;
    pub fn snapshot(&self) -> OpeningSnapshot;
    pub fn default_next_color(&self) -> Option<Color>;
    pub fn place(&mut self, mv: Move, color: Option<Color>) -> Result<Vec<Event>, CommandError>;
    pub fn undo(&mut self, n: u32) -> Result<Vec<Event>, CommandError>;
    pub fn done(&mut self) -> Result<Vec<Event>, CommandError>;
    pub fn more(&mut self) -> Result<Vec<Event>, CommandError>;
    pub fn choose(&self, color: Color) -> Result<Board, CommandError>;
}

pub struct OpeningSnapshot {
    pub phase: OpeningPhase,
    pub placements: Vec<(Move, Color)>,
    pub default_next_color: Option<Color>,
}
```

The module is separate from `session.rs` for the design paper's own
size rule: `session.rs` was already past the budget, and the ceremony
is one concern with one invariant set. The module speaks the session's
event vocabulary directly; there is no second event language to map.

Details that matter:

- The default color alternates **within each round**, black first.
  The count restarts per round: the fourth stone overall is the first
  stone of round two and defaults to black. A global parity rule is
  wrong at exactly that stone; the in-module test pins the restart.
- `default_next_color` returns `None` at a choice phase. There is no
  next stone there, and a made-up default would put a false "next:
  black" on the screen.
- The rebuild helpers (`rebuild_placing3`, `rebuild_placing2`,
  `rebuild_first_choice`, `rebuild_final_choice`) replay the log into
  the typestate. Their `expect` calls document invariants ("the log
  contains only engine-validated placements"); they are not error
  handling.
- `choose` takes `&self` and returns the finished `Board`. The session
  decides what the board becomes (the tree root) and clears the
  opening state. The machine itself changes nothing at the boundary.

### `src/session.rs`

- `SessionInit` gains `swap2: bool` (all existing constructors set it
  to `false`) and a chainable `with_swap2(bool)`.
- `SessionCore::new` asserts that a Swap2 session starts from an empty
  opening board. The combination is a programming error — ceremony
  counts can be illegal in a game position — so it panics instead of
  returning a typed error. User input never reaches this constructor.
- `SessionCore` gains `opening: Option<OpeningState>`. While it is
  `Some`, `execute` routes `play`, `back`, `done`, `more`, `black`,
  and `white` into the machine, and `execute_step` refuses with
  `OpeningInProgress`.
- `choose` ends the ceremony: the returned board becomes
  `History::from_opening(&board)`, `opening` becomes `None`, and the
  normal command set takes over.
- `reset` restores the ceremony when `init.swap2` is set.
- `Snapshot` gains `opening: Option<OpeningSnapshot>`. Serde writes
  the field only when it is `Some`, so the wire format of an ordinary
  session is byte-identical to before; old snapshots without the field
  still deserialize (`None`).

### `src/repl.rs`

The help text gains a Swap2 block. `print_state` prints one extra line
during the ceremony — phase, placements in order, and the default next
color — and nothing otherwise. Ordinary transcripts are byte-identical
to before; the demo scripts prove it.

### Tests

- `tests/opening.rs` (new): the full protocol paths (take black, take
  white, `more` then choose), undo inside and across rounds, `back N`,
  count validation at `done`, occupied-square rejection, the color
  override, the default alternation, every typed error in and out of
  phase, `step` rejection, `new` semantics, the snapshot contents, the
  serde round trip, tree navigation after the ceremony, and the
  construction panic.
- `src/opening.rs` unit tests pin the per-round alternation, the
  undo-to-phase rule, and the snapshot mirror.
- `src/command.rs` unit tests cover the new syntax: the color token on
  both `play` forms, the four new commands, and their argument
  errors.
- `tests/common/mod.rs`: `play` builds the struct variant; new helpers
  `play_color` and `swap2_core`; `event_type` knows the four new
  events.

## Solution (opt-in)

The added and changed code, as annotated snippets with line numbers, is
in [swap2-solution.md](swap2-solution.md). Open it only if you have
been stuck for more than twenty minutes, or after you have finished
the sub-chapter and want to compare your work.

## TDD checklist

The rhythm is red, green, extend. Run from `gomoku/`.

1. **Vocabulary.** Add the `Play` struct variant, the four new
   commands, `OpeningPhase`, the events, and the errors to
   `command.rs`. Fix every construction site (the test kit absorbs
   most). Add the parser tests and watch them fail — the parser does
   not know the tokens yet. Implement the parser arms and
   `parse_optional_color`. Green.
2. **The machine.** Write the failing tests for the default
   alternation (including the per-round restart) and the undo rule
   against `OpeningState`. Implement `src/opening.rs`. Green.
3. **Routing.** Write the failing integration tests in
   `tests/opening.rs`: the take-black path first, then the errors.
   Wire `SessionInit.swap2`, the `opening` field, and the phase
   routing in `session.rs`. Green.
4. **The rest of the protocol.** Extend the integration tests to the
   take-white path, the `more` path, undo across rounds, `back N`,
   and the construction panic. Extend the implementation where a test
   demands it. Green.
5. **The seam.** Add the snapshot and serde tests, then the `Snapshot`
   field and the repl line. Run the gates and both transcript diffs.

## Verification commands

From `gomoku/`:

```text
cargo test -p cli
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Plus the transcript capture and diff from sub-chapter a's checklist.
Both diffs must be empty; neither script uses Swap2.

## Done when

* `play` carries the optional color token, and the four ceremony
  commands parse and execute.
* `src/opening.rs` holds the replay log and rebuilds the engine
  typestate for every rule decision. No Swap2 rule is re-implemented
  in the cli crate.
* `Snapshot` exposes the ceremony (phase, placements, default color),
  and ordinary sessions serialize byte-identically to before.
* After `OpeningFinished`, the tree root is the finished opening board
  and White moves first.
* `new` restarts the ceremony in a Swap2 session.
* All three gates pass, and both demo transcripts are unchanged.
  (Result here: 101 tests, up from 74.)

Next: sub-chapter d — the alternate-screen shell.

## References

* [00-design.md](../00-design.md) — the contract for all six
  sub-chapters, including the Swap2 vocabulary and the free placement
  order.
* [a-refactor/refactor.md](../a-refactor/refactor.md) — the test kit
  and the transcript-diff technique this sub-chapter reuses.
* [b-renderer/renderer.md](../b-renderer/renderer.md) — the renderer
  that will draw the ceremony from `Snapshot.opening`.
* [docs/13-engine-design.md](../../../../docs/13-engine-design.md) —
  the Swap2 typestate decision and the absolute-colors decision.
