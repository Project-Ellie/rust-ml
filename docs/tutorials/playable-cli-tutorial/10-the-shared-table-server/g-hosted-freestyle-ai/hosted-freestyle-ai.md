# 10g — The hosted freestyle AI

## Intent

Seat the existing tactics-driven MCTS opposite a verified owner without
blocking Tokio and without making recovery rerun search. AI tables are
freestyle-only in v1: the owner chooses a color, and the hosted AI takes
the other.

The capstone survives actor eviction with the exact selected move and
search record intact.

## Why `Command::Step` is not enough by itself

Today `SessionCore::execute(Command::Step)` runs MCTS synchronously,
plays the move, and stores `SearchInfo` at the resulting history node.
Calling it directly in the async actor would block a Tokio worker.
Replaying it during recovery would run a new search and might select a
different move.

Keep that tested behavior, but add one narrow recovery seam to
`SessionCore`: apply an already selected AI move together with its
`SearchInfo`, producing the same move and game-over events without
running MCTS. This is not a second game model; it is deterministic input
for the existing history and search-info storage.

The seam must validate that:

- the game is ongoing;
- no Swap2 ceremony is active;
- the side to move is an AI occupant;
- the move is legal;
- the search record describes that side to move.

## Live AI execution

When it becomes the AI's turn:

1. the actor acquires a permit from the global AI semaphore;
2. it moves ownership of its `SessionCore` into `spawn_blocking`;
3. the blocking closure executes the existing `Step` command;
4. it returns the modified core, selected move, emitted events, and
   cloned `SearchInfo`;
5. the actor maps `SearchInfo` to `SearchInfoDto`;
6. it atomically commits a canonical AI journal entry and client events;
7. only then does it install the modified core and publish.

The mailbox remains bounded while the actor waits. No other mutation for
that table can overtake the AI turn.

## Resource policy

- one global semaphore bounds concurrent searches across all tables;
- table creation and configuration enforce a hard maximum simulation
  count;
- named presets provide friendly defaults;
- once a search starts, it is not cancelled because a client disconnects;
- server shutdown waits for a bounded grace period, then reports any
  unfinished blocking work rather than pretending it was committed.

No wall-clock search budget or unsafe thread cancellation exists in v1.

## Canonical journal entry

A live AI step is recorded as data, not intent:

- selected move;
- `SearchInfoDto` containing side to move, visits, prior, value, and
  effective AI config;
- server-generated `CommandId`;
- committed `TableRevision`.

Recovery maps the DTO back to `cli::SearchInfo` and invokes the
recorded-AI seam. It must not call `mcts::search`, acquire the semaphore,
or consume random numbers.

## Automatic advancement

After any committed mutation, the actor asks whether the active Live
position belongs to the hosted AI. If yes, it enqueues or begins one AI
turn. It repeats only after each successful commit and stops when:

- it becomes the owner's turn;
- the game ends;
- the table leaves Live mode;
- persistence fails.

Because an AI table always contains the owner in the opposite seat, this
loop never becomes unbounded AI-versus-AI selfplay.

## TDD checklist

1. Swap2 plus AI is rejected at table creation.
2. owner and AI occupy opposite freestyle colors.
3. no configuration can replace the owner with AI.
4. MCTS runs on a blocking thread, not a Tokio worker.
5. the global semaphore caps simultaneous searches across tables.
6. simulations above the hard maximum are rejected.
7. client disconnect does not cancel a started search.
8. live search commits selected move and complete `SearchInfoDto`.
9. persistence failure installs neither move nor search record.
10. replay invokes the deterministic seam and a test evaluator proves
    MCTS was not called.
11. recovered `search_info()` matches the original search record.
12. an AI terminal move emits and persists game-over state once.

## Gates and capstone

Run focused `cli` tests for the deterministic replay seam, server tests
for blocking and persistence behavior, then workspace tests, clippy, and
formatting. The capstone plays several owner-versus-AI turns, records the
AI overlay data, evicts the actor, recovers it, and renders the same
selected move, visits, prior, and value without another search.
