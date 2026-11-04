# 10j — Two-table acceptance

## Intent

Prove the server as a system, not a collection of green units. One
verified owner and anonymous guest play a human Swap2 table while a
second verified owner plays a freestyle table against the hosted AI.
Both tables stream concurrently, survive actor eviction, recover from
Redis, and retain their isolation.

The target scale remains modest: one process, roughly 100 active tables,
200 SSE clients, and bounded AI work. This chapter proves that design;
it does not pretend to certify an internet-scale service.

## Acceptance topology

Run real components wherever their behavior matters:

- one `gomoku-server` process on an ephemeral port;
- one disposable Redis instance;
- `ConsoleMailer` captured by a test harness;
- owner A and guest A clients;
- owner B client with hosted AI opponent;
- real `SessionCore` and tactics evaluator with a small deterministic
  simulation preset;
- injected clock for TTL and idle-eviction scenarios.

Do not mock Redis in this suite. Do not depend on external SMTP or a
public network.

## Story A — Human Swap2 table

1. Owner A requests and follows a magic link.
2. The verified owner creates a human Swap2 table.
3. Owner A creates one invitation; guest A redeems it.
4. Both clients open SSE streams.
5. They complete the ceremony as color-neutral participants.
6. The final choice assigns opposite colors.
7. Each participant plays only their assigned turns.
8. Owner A enters Study mode; guest A observes and cannot mutate.
9. The owner creates and selects a branch, then resumes at its tip.
10. Both clients agree on snapshot and revision.

## Story B — Owner versus hosted AI

1. Owner B verifies independently.
2. Owner B creates a freestyle AI table and chooses a color.
3. No invitation can be issued for this table.
4. Owner B and the AI alternate several moves.
5. Each AI turn records the selected move and `SearchInfoDto`.
6. Owner B's SSE stream sees each committed revision exactly once.

## Recovery and isolation story

1. Record both snapshots and revisions.
2. Advance the fake clock beyond the 15-minute actor idle interval.
3. Assert both registry handles disappear while Redis payloads remain.
4. Access both tables concurrently and force actor reconstruction.
5. Assert snapshots, hosted states, assignments, branches, and revisions
   match the recorded values.
6. Assert AI reconstruction performed no MCTS search.
7. Reuse a committed `CommandId` and receive its original result without
   another move.
8. Reuse that id with different input and receive 409.
9. Present guest A's bearer to table B and receive 403.
10. Verify activity on table A does not refresh table B's TTL.

## Lifecycle story

- Owner A explicitly closes table A. Reads, commands, and SSE reconnects
  return 410; its participant credentials no longer authorize it.
- Advance table B beyond its seven-day inactivity deadline. Its payload
  is removed, its compact lifecycle record identifies it as expired,
  and reads, commands, and streams return 410.
- A random never-issued table id returns 404.

## Failure story

Exercise one persistence fault while a legal move is provisional:

1. arrange Redis commit failure after `SessionCore` execution;
2. assert the HTTP response is 503;
3. assert no SSE envelope was published;
4. assert the actor projection is discarded;
5. restore Redis and retry with the same `CommandId`;
6. reconstruct from the last committed journal and commit the move once.

Also saturate the global AI semaphore and one table mailbox. The server
must remain responsive to health checks and unrelated table reads.

## Security assertions

Search structured application logs and HTTP responses for the following.
The harness captures the dedicated `ConsoleMailer` delivery sink
separately because that sink intentionally contains the magic link.

- raw magic tokens;
- raw invitation tokens;
- owner and guest bearers;
- full email addresses;
- Redis credentials.

The test fails if any appears. It also proves auth and redemption rate
limits, request body limits, and the loopback-or-TLS deployment rule.
This is still friendly-game security, not production identity review.

## Performance observation

Run a bounded soak with 100 active actor handles and 200 SSE clients.
Record rather than overfit:

- command latency without AI;
- AI queue wait and search duration;
- Redis commit latency;
- mailbox saturation count;
- SSE disconnect/reset count;
- memory before and after idle eviction.

The acceptance gate is bounded resource behavior and no cross-table
interference, not a universal latency number.

## Final verification matrix

| Property | Evidence |
|---|---|
| verified owners create tables | auth and creation transcript |
| invitation is one-time and scoped | redemption and cross-table test |
| actor exclusively owns core | actor API and concurrent command test |
| Redis is authoritative | eviction and reconstruction test |
| AI replay is deterministic | zero-search recovery assertion |
| study is owner-only | guest 403 plus continued SSE observation |
| retries are idempotent | same-id and conflicting-id tests |
| reconnect is lossless | replay and reset tests |
| lifecycle is visible | 410 closed/expired and 404 unknown |
| limits are bounded | soak observations and saturation tests |

## Gates and capstone

Run the complete protocol, cli, and server suites, then workspace tests,
clippy with warnings denied, formatting check, and `git diff --check`.
The final capstone is a saved acceptance transcript showing both tables
progressing at once, actor eviction and recovery, an AI search record
surviving recovery, table A closing, and table B expiring.
