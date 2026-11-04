# 10d — Redis is the table

## Intent

Make the actor disposable. Every accepted mutation becomes a canonical
journal entry in Redis, every resulting client event enters a separate
bounded stream, and a fresh actor can reconstruct the same table without
rerunning side effects.

The capstone is recovery: stop an actor, start another, and obtain the
same snapshot and revision from Redis alone.

## Two streams, two jobs

Do not ask one record format to serve incompatible purposes.

| Redis structure | Retention | Consumer | Contents |
|---|---|---|---|
| command journal | durable for table lifetime | actor recovery | accepted canonical core commands and hosted transitions |
| client-event stream | bounded recent window | SSE reconnect | one `EventEnvelope` per committed revision |

`cli::Event` cannot be executed by `SessionCore`. Therefore event
records are never the recovery log. Recovery maps deterministic journal
entries back to `cli::Command` and applies hosted transitions to actor
state.

A journal entry records enough context to be deterministic:

- `CommandId` and request digest;
- committed `TableRevision`;
- authenticated participant role, not a raw bearer;
- canonical command or hosted transition;
- for hosted AI only, selected move plus `SearchInfoDto`;
- schema version.

## Revision is not a stream id

`TableRevision` is an integer in table metadata. One successful table
mutation increments it exactly once. Store that integer as a field in
journal entries, event envelopes, snapshots, and idempotency results.

Redis Stream IDs remain storage cursors. They are useful for reading a
stream, but they are not the public concurrency version. A Lua commit
must not infer revision from `XLEN`: trimming the event stream would
break that equation.

## One atomic commit

The repository exposes typed operations; handlers and actors do not
issue ad-hoc Redis commands. The central Lua-backed operation receives:

- table id;
- expected revision;
- command id and request digest;
- canonical journal payload;
- one `EventEnvelope` containing zero or more `WireEvent`s;
- response payload;
- current time and TTL policy.

It atomically:

1. checks the compact lifecycle record and active payload;
2. returns the stored result if the same command id and request digest
   already committed;
3. rejects the same id with different input;
4. compares expected and current revision;
5. increments the integer revision;
6. appends the canonical journal entry;
7. appends the `EventEnvelope` to the bounded event stream;
8. stores the idempotent response;
9. refreshes the table payload's seven-day TTL;
10. returns the committed result.

Only after this succeeds may the actor answer the command or publish to
live SSE subscribers.

## Failure after local execution

`SessionCore::execute` mutates before Redis knows whether persistence
will succeed. Treat the modified core as provisional:

- commit succeeds: install it as the actor's current projection and
  publish committed events;
- commit fails: publish nothing, drop or poison the actor projection,
  and terminate the actor;
- next request: the registry starts a new actor and replays only the
  committed journal.

Never continue using a core that contains an uncommitted move.

## Expiry and 410

The table payload has a sliding seven-day Redis TTL. A successful
authorized snapshot read, event-stream connection, or committed
mutation refreshes it; heartbeats and failed requests do not. Keep a
compact lifecycle catalog or gone tombstone after payload removal so a known
closed or expired id returns `410 Gone`, while a never-issued id returns
`404 Not Found`. Explicit closure atomically marks the tombstone and
revokes table-scoped credentials.

The small lifecycle record is not the game state. The command journal,
event window, idempotency records, and table payload may be deleted when
the table expires.

## Recovery algorithm

1. load metadata and lifecycle status;
2. construct the original `SessionInit`;
3. create a fresh `SessionCore`;
4. read canonical journal entries in revision order;
5. verify contiguous integer revisions;
6. replay deterministic core commands and hosted transitions;
7. replay canonical AI moves through the dedicated seam added in 10g;
8. verify that replay reached the metadata's committed revision;
9. register the actor only after recovery succeeds.

Corrupt or unsupported journal data fails closed with 503 and a
high-severity diagnostic; it is never skipped.

## TDD checklist

1. one commit writes journal entry, `EventEnvelope`, idempotency result,
   revision, and refreshed TTL atomically.
2. a stale expected revision writes nothing.
3. the same `CommandId` and request digest returns the original result.
4. the same `CommandId` with different input returns 409.
5. trimming client events does not affect actor recovery.
6. simulated Redis failure after local execution publishes nothing and
   forces actor replacement.
7. replay produces the same snapshot and hosted state.
8. missing or duplicated revisions fail recovery.
9. actor eviction followed by access recovers the table.
10. expired and closed ids return 410; unknown ids return 404.

Use an isolated Redis database or disposable Redis container. Flush only
the test-owned instance.

## Gates and capstone

Run server integration tests serially where they share Redis, then the
normal workspace lint and format gates. For the capstone, commit a line
with a branch, kill the actor, trim the entire client-event window, and
recover the same branch-aware snapshot solely from the command journal.
