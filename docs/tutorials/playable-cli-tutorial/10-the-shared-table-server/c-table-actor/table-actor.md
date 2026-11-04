# 10c — One table, one actor

## Intent

Put one existing `SessionCore` behind one bounded mailbox. At the end of
this slice, a verified owner can create an in-memory table and submit a
core command through the actor handle. There is still no durable command
journal; persistence arrives in 10d.

This slice teaches the central concurrency decision: serialize a
table's mutations by ownership, not by locking its state.

## The ownership story

`SessionCore::execute(&mut self, Command)` already wants exclusive mutable
access. Give it exactly that:

- one `TableActor` task owns one `SessionCore`;
- the actor also owns hosted state absent from the core: owner identity,
  guest slot, participant-to-color assignment, live/study mode, and
  current `TableRevision`;
- callers hold a cloneable `TableActorHandle`, not the core;
- the handle sends a message through a bounded Tokio mpsc channel and
  waits on a oneshot reply;
- no `Arc<Mutex<SessionCore>>` exists.

The actor is a consistency boundary. Invitation redemption, study-mode
changes, core commands, closure, and automatic AI moves will all enter
through this same mailbox in later slices.

## Actor message contract

A mutation message carries:

- authenticated principal (`Owner` or table-scoped `Guest`);
- `CommandId`;
- expected `TableRevision`;
- a protocol-owned mutation;
- oneshot response channel.

The actor performs checks in this order:

1. table is not closed or expired;
2. `CommandId` is not a conflicting reuse;
3. expected revision matches;
4. principal is permitted in the hosted mode;
5. protocol mutation maps to a valid `cli::Command` or hosted transition;
6. `SessionCore::execute` accepts the command;
7. return mapped events and snapshot.

In 10c, the last result is provisional and memory-only. In 10d, step 7
becomes a Redis commit before any response or broadcast.

## Table registry

`TableRegistry` maps `TableId` to actor handles. Its responsibilities are
narrow:

- return the existing live handle;
- ensure concurrent misses start at most one actor;
- remove a handle when its actor exits;
- evict an actor after 15 minutes without mailbox activity;
- permit a later request to rebuild the actor.

The registry does not decide game legality and does not expose core
references. Idle eviction is not table expiry: Redis keeps the table for
seven sliding days, while the in-memory projection may disappear after
15 minutes.

## Authorization at the actor boundary

HTTP middleware proves who sent a bearer. The actor decides what that
principal may do in this table. Keeping both checks prevents a future
non-HTTP caller from bypassing table authorization.

For this slice, implement owner-only freestyle commands. Reserve the
following policy for 10f:

- a guest acts only for their assigned participant in Live mode;
- only the owner enters Study mode and acts for either side there;
- Help and Quit never reach the actor.

## Backpressure

The mailbox is bounded. If it is full, handlers fail quickly with a
stable overloaded response rather than accumulating unbounded work.
Choose the capacity from the design scale—100 active tables and 200 SSE
clients—and expose saturation in metrics. Capacity is a resource limit,
not a correctness mechanism.

## TDD checklist

1. one handle processes two concurrent commands in mailbox order.
2. no test or public API can obtain `&mut SessionCore` from a handle.
3. a stale expected revision returns 409 without executing the command.
4. reusing a `CommandId` with different input returns 409.
5. a `SessionCore` rejection maps to 422 and leaves revision unchanged.
6. an unauthorized principal returns 403 without touching the core.
7. a full mailbox returns the chosen overloaded status.
8. concurrent registry misses produce one actor.
9. fake-clock inactivity evicts an actor after 15 minutes.
10. actor exit removes its registry handle.

## Error boundary

Actor transport failure is not a game error:

- mailbox saturated: `429 Too Many Requests`;
- actor unavailable or unexpectedly stopped: `503 Service Unavailable`;
- revision or idempotency conflict: `409 Conflict`;
- forbidden table action: `403 Forbidden`;
- rejected core command: `422 Unprocessable Entity`.

## Gates and capstone

Run `cargo test -p server`, the protocol tests, and the workspace lint
and format gates. The capstone launches one actor, sends a short legal
line from concurrent test tasks, and proves the resulting move order is
the mailbox order. Then advance a fake clock and show the registry evicts
the actor without declaring the table expired.
