# Chapter 10 — The shared table server

**Status:** tutorial design complete (2026-11-03). This chapter captures
the approved online-server design and ten learner-facing implementation
slices. It does not ship server code or opt-in solutions: the owner can
read the whole system and its tests before writing the first crate.

## Purpose

The local `cli` crate already has a headless `SessionCore` that speaks
`Command` and `Event`. This chapter extends that core into a hosted,
multi-table service: owners create tables on a server, invite guests,
and play from separate terminals or browsers while the server persists
every move in Redis. The AI can still take a seat, but now it thinks on
a worker thread and publishes its search record to every observer.

By the end of the design you should be able to explain:

- how a verified owner, a magic link, and a table-scoped bearer token
  fit together;
- how one `TableActor` owns exactly one `SessionCore` and why the actor
  is allowed to crash without losing the game;
- why the Redis command journal is authoritative and the actor is only
  a cache;
- how owner-only Study Mode differs from ordinary live play;
- how REST commands and SSE event streams keep every client in sync;
- where the security boundary is and why this is a friendly-game system,
  not a production identity system.

## Prerequisites

You need the playable CLI tutorial up to chapter 09:

- `SessionCore`, `SessionInit`, `Command`, `Event`, `Snapshot`,
  `CommandError`, and the core/shell split ([primer](00-playable-cli-primer.md)).
- `Occupant`, `AiConfig`, `SearchInfo`, `SeatSel`, and the AI seat
  ([chapter 03](03-the-ai-takes-a-seat.md), [chapter 04](04-the-study-loop.md)).
- The Swap2 opening and `OpeningState`
  ([chapter 05](05-the-real-table.md)).
- Basic familiarity with Axum, Tokio, and Redis. You do not need to
  have written a REST server before; the design names the exact
  responsibilities of each layer.

## Architecture at a glance

```text
                         ┌─────────────────┐
   gomoku play  ────────▶│   cli crate     │  local, independent
                         │  SessionCore    │
                         └─────────────────┘
                                    ▲
                                    │ uses
   ┌──────────────┐    ┌────────────┴────────────┐
   │   protocol   │◄───│      gomoku-server      │ Axum + Tokio + Redis
   │    crate     │    │  TableActor, TableRegistry, Mailer
   └──────────────┘    └─────────────────────────┘
```

- **`gomoku-server`** is a new standalone executable. It depends on the
  existing `cli` crate for `SessionCore`, a new `protocol` crate for
  wire types, and external Redis for persistence.
- **`protocol`** is shared between server and future remote clients. It
  defines table identifiers, command payloads, event envelopes, session
  tokens, and error bodies.
- **`TableActor`** is a Tokio task that owns one `SessionCore`. All
  commands to a table go through its bounded mailbox; no mutex protects
  the core.
- **`TableRegistry`** keeps the actor handles and makes sure a table is
  loaded into memory when a request arrives.
- **Redis** stores an append-only canonical command journal and a
  bounded client-event stream for every table, plus owner verification
  state, bearer-token digests, and invitation digests. Redis is the
  source of truth; the actor can be rebuilt from the command journal at
  any time.
- The existing `gomoku` binary keeps its local `play` and `selfplay`
  modes untouched. A future `gomoku remote` mode will speak the
  protocol to the server, but that is not part of this design
  milestone.

## Chapter map

The design is split into ten subchapters so the implementation can be
worked one slice at a time:

| Subchapter | Title | What it designs |
|---|---|---|
| a | [Protocol and server shell](10-the-shared-table-server/a-protocol-and-server-shell/protocol-and-server-shell.md) | The `protocol` crate, the `gomoku-server` binary entry point, and the Axum router skeleton. |
| b | [Verified owner](10-the-shared-table-server/b-verified-owner/verified-owner.md) | Account verification, the `Mailer` trait, `ConsoleMailer`, magic links, and the 30-day sliding bearer session. |
| c | [Table actor](10-the-shared-table-server/c-table-actor/table-actor.md) | `TableActor`, bounded mailbox, `TableRegistry`, actor eviction, and exclusive `SessionCore` ownership. |
| d | [Redis journal](10-the-shared-table-server/d-redis-journal/redis-journal.md) | Canonical command journal, bounded event stream, recovery, Lua commits, and failure semantics. |
| e | [Invitation](10-the-shared-table-server/e-invitation/invitation.md) | The 256-bit URL-safe invitation token, digest storage, table and guest binding, one-time redemption, and the guest bearer. |
| f | [Swap2 and Study Mode](10-the-shared-table-server/f-swap2-and-study/swap2-and-study.md) | Human Swap2 with color-neutral participants, freestyle assignments, and owner-only Study Mode. |
| g | [Hosted freestyle AI](10-the-shared-table-server/g-hosted-freestyle-ai/hosted-freestyle-ai.md) | Owner-versus-AI freestyle, `spawn_blocking`, bounded search, canonical results, and deterministic recovery. |
| h | [REST and SSE completion](10-the-shared-table-server/h-rest-and-sse/rest-and-sse.md) | Endpoint contract, SSE reconnection, reset snapshots, expected revision, and `CommandId` idempotency. |
| i | [Optional remote CLI](10-the-shared-table-server/i-remote-cli/remote-cli.md) | An explicit `gomoku remote` shell that leaves local `gomoku play` independent. |
| j | [Two-table acceptance](10-the-shared-table-server/j-two-table-acceptance/two-table-acceptance.md) | Simultaneous owner-plus-guest and owner-plus-AI tables, recovery, isolation, expiry, and 410 behavior. |

The authoritative design paper for all ten subchapters is
[`10-the-shared-table-server/00-design.md`](10-the-shared-table-server/00-design.md).

## Glossary

| Term | Definition |
|---|---|
| **Owner** | The person who created a table. Verified by email+bearer. Can act, invite, and enter Study Mode. |
| **Guest** | A human invited to a specific table. Anonymous except for a table-scoped bearer token. Observes in Study Mode; acts only in Live Mode on their turn. |
| **Table** | A hosted game or study session. Has a mode, an owner, optionally a guest, and a command journal. |
| **TableActor** | One Tokio task that owns one `SessionCore` and processes commands from a bounded mailbox. |
| **TableRegistry** | In-memory registry of running `TableActor` handles, indexed by table id. |
| **Verified owner** | An owner who has clicked a magic link within 15 minutes and holds a sliding 30-day bearer token. |
| **Magic link** | A passwordless verification URL sent to the owner's email. The raw token is not stored; only its cryptographic digest is kept in Redis. |
| **Invitation token** | A 256-bit URL-safe token handed to the owner to share with a guest. Only its cryptographic digest is stored. |
| **Owner bearer** | A long-lived session token for the owner, scoped to the whole server, sliding 30 days. |
| **Guest bearer** | A table-scoped anonymous bearer issued when an invitation is redeemed. |
| **CommandId** | A client-supplied idempotency key for a command. Replaying the same id returns the same result without mutation. |
| **TableRevision** | The monotonic integer version committed for each table mutation. Used for SSE ordering and optimistic concurrency; distinct from a Redis Stream ID. |
| **Command journal** | The append-only Redis Stream of accepted canonical commands and hosted state transitions for one table. |
| **Client-event stream** | A bounded Redis Stream of response envelopes used for SSE reconnection; it is not replayed into `SessionCore`. |
| **EventEnvelope** | One committed table revision together with all wire events emitted by that mutation; the atomic SSE unit. |
| **Reset snapshot** | A full `Snapshot` sent to an SSE client when it reconnects with an outdated Last-Event-ID. |
| **Study Mode** | Owner-only mode where the owner can navigate and play both sides; the guest observes and cannot act. Resuming live play starts from the selected branch tip. |
| **Live Mode** | Normal play. The owner and guest act according to their seats and whose turn it is. |
| **Human table** | A table with two human seats. Can be Swap2 or freestyle. |
| **AI table** | A table with the verified owner in one seat and the hosted AI in the other. Freestyle-only in v1; it has no guest slot. |
| **Freestyle** | Ordinary Gomoku. The owner chooses their color when creating the table. |
| **Swap2** | The Swap2 opening ceremony. Participants are color-neutral until the final choice. |
| **ConsoleMailer** | The only v1 mailer implementation. It prints the magic link to stdout. Hidden behind the `Mailer` trait so an `SmtpMailer` can be added later. |
| **Redaction** | Sensitive fields, such as raw tokens and email addresses, are removed from logs and error responses. |
| **Friendly-game security boundary** | The system protects against casual abuse and accidental leakage, but it is not production identity. |

## Completion target

After this documentation milestone you can describe the full hosted
system, justify every major decision, and work it as ten vertical TDD
slices. You do **not** build it in this pass: no server crate, protocol
implementation, or changes to `cli` or `gomoku` are included.

The design paper and subchapters are complete teaching contracts, not
placeholders. Implementation notes and opt-in solutions are intentionally
deferred until each slice has been built and verified against live code.
