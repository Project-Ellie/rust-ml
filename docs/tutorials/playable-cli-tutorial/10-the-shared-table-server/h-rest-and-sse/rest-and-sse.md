# 10h — REST writes, SSE facts

## Intent

Complete the network shell. Clients submit mutations through REST,
receive committed results, and follow the table through one
server-sent-event stream. Reconnection uses `Last-Event-ID`; stale
clients receive a reset snapshot rather than an unbounded replay.

REST carries intent. SSE carries committed facts. Neither transport owns
the table.

## Endpoint surface

All endpoints live below `/v1`.

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/auth/magic-links` | request owner verification |
| `GET` | `/auth/verify` | consume magic token and issue owner session |
| `POST` | `/tables` | verified owner creates a human or AI table |
| `GET` | `/tables/{id}` | fetch authorized `TableSnapshot` |
| `POST` | `/tables/{id}/invitations` | owner creates a human invitation |
| `POST` | `/invitations/redeem` | guest atomically joins a human table |
| `POST` | `/tables/{id}/commands` | submit a core command or hosted transition |
| `GET` | `/tables/{id}/events` | open authorized SSE stream |
| `GET` | `/health/live` | process liveness |
| `GET` | `/health/ready` | Redis readiness |

Every table mutation carries a `CommandId`. Mutations of an existing
table also carry its expected `TableRevision`; creation has an implicit
starting revision of zero. The server returns the committed revision,
wire events, and current snapshot. Automatic AI mutations use server-generated command ids.

## Authorization

Use `Authorization: Bearer ...`; never put owner or guest bearers in a
URL. Owner sessions authenticate an account, then table authorization
checks ownership. Guest sessions authenticate one guest and carry one
table scope.

SSE is authorized exactly like snapshot reads. A guest remains allowed
to observe during Study mode even though guest mutations are forbidden.

Production-like deployments terminate TLS before bearer traffic. Plain
HTTP is acceptable only on loopback during the tutorial. CORS is denied
by default, bodies are size-limited, and logs redact authorization and
magic-link query values.

## SSE envelope

Each SSE message has:

- `id`: decimal `TableRevision`;
- `event`: stable event kind such as `table_events` or `reset`;
- `data`: one JSON commit envelope.

A table mutation can emit multiple `WireEvent`s. Keep all events from
one mutation together in the single commit envelope for its revision.
That makes the SSE id unique and preserves the mutation's atomic unit: a
reconnecting client either applies the whole revision or none of it.

Heartbeat comments keep intermediaries from considering an idle stream
dead. Heartbeats are not persisted and do not advance revision.

## Reconnection

When `Last-Event-ID` is present:

1. parse it as `TableRevision`, never as a Redis Stream ID;
2. authorize the principal;
3. compare it with current revision and the oldest retained event
   revision;
4. replay complete later revisions if they remain in the bounded event
   stream;
5. otherwise send one `reset` envelope with the current
   `TableSnapshot` and revision;
6. subscribe to live committed events without a gap.

The repository must establish replay-plus-subscribe atomically enough
that a commit cannot fall between the two. One approach is subscribe,
read the committed window through a captured revision, then drain live
messages with de-duplication. The implementation must prove the no-gap
property in a race test.

## Idempotency

Before executing a mutation, the actor checks Redis for its `CommandId`:

- same id and same request digest: return the committed result;
- same id and different request: 409 Conflict;
- unseen id: continue through authorization, execution, and atomic
  commit.

The cached result includes the exact committed revision and response
events. Secret-issuing flows use purpose-built records and never store a
raw bearer in a generic idempotency body.

## Stable error mapping

| HTTP | Use |
|---|---|
| 400 | malformed JSON, identifier, or command shape |
| 401 | absent, invalid, or expired bearer |
| 403 | valid principal lacks permission or table scope |
| 404 | resource was never known |
| 409 | stale revision or conflicting `CommandId` reuse |
| 410 | known table or invitation is closed, expired, or consumed |
| 422 | well-formed mutation rejected by table or `SessionCore` rules |
| 429 | rate or mailbox limit exceeded |
| 503 | Redis, actor recovery, or required dependency unavailable |

Every error body has a stable code, human message, and optional safe
details. It never contains Redis diagnostics, emails, or raw tokens.

## TDD checklist

1. every endpoint enforces its principal and scope.
2. command response is sent only after Redis commit.
3. SSE publishes only committed envelopes.
4. reconnect within the window receives each later commit envelope once.
5. reconnect before the window receives one reset snapshot.
6. all events from one revision remain in one atomic SSE envelope.
7. a commit racing stream startup is neither lost nor duplicated.
8. repeated command id returns the original result.
9. conflicting command-id reuse returns 409.
10. closed and expired tables return 410 for reads, writes, and streams.
11. slow SSE consumers are disconnected or reset without unbounded
    memory growth.
12. rate limits protect auth, invitation redemption, commands, and SSE
    connection attempts.

## Gates and capstone

Run protocol, server unit, Redis integration, and HTTP black-box tests,
then workspace clippy and formatting. The capstone opens two clients,
plays through REST, disconnects one stream, advances the table, and
reconnects first within and then beyond the retained window to observe
both replay and reset behavior.
