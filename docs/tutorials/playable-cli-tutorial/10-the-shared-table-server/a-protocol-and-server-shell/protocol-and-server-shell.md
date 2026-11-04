# 10a — Protocol and server shell

## Intent

Build the boundary before the behavior. At the end of this slice, a new
`protocol` crate owns the JSON vocabulary, a new `server` library owns
application assembly, and the standalone `gomoku-server` binary can
report whether it can reach Redis. No table can be created yet.

The felt result is deliberately small: one process starts, one health
request succeeds, and neither the wire contract nor the existing
`SessionCore` depends on Axum.

## Start from the dependency rule

The target dependency graph is:

```text
protocol                 serde data, no Axum, Tokio, Redis, cli, or server
    ▲       ▲
    │       └── server   translates DTOs and hosts SessionCore
    └────────── cli      used only by the later explicit remote mode

server ─────────► cli    reuses SessionCore
```

This direction matters more than the first endpoint. If `protocol`
depends on `cli`, adding `cli -> protocol` in 10i creates a cycle. If
wire DTOs derive Axum types, every client inherits the web framework.
Keep `protocol` boring and portable.

## What to build later

Add two workspace crates during implementation:

- `protocol`: identifiers, request/response DTOs, wire commands, wire
  events, snapshots, stable error codes, and serde tests;
- `server`: an Axum router, application state, Redis repository seam,
  and the `gomoku-server` binary target.

Do not expose `cli::Command` or `cli::Event` directly. They are not
serde wire types today. Define protocol-owned equivalents and put the
mapping in `server`. Although `cli::Snapshot` is serializable, the
remote read model also needs hosted fields such as participant identity,
mode, revision, and study state; that belongs in `TableSnapshot`.

The initial identifiers are opaque newtypes:

- `TableId`
- `CommandId`
- `TableRevision`
- `OwnerId`
- `GuestId`

The initial top-level DTOs are:

- `CreateTableRequest` and `CreateTableResponse`
- `MutationRequest` and `MutationResponse`
- `WireCommand`, `WireEvent`, and atomic `EventEnvelope`
- `TableSnapshot` and `TableSummary`
- `ApiError` with a stable machine-readable code

Secret bearer values need redacting `Debug`; plain strings do not.
Generation and digesting of secrets stay in `server`, not `protocol`.

## First HTTP contract

Only health is live in this slice:

| Method | Path | Meaning |
|---|---|---|
| `GET` | `/health/live` | The process is running; no dependency check. |
| `GET` | `/health/ready` | Redis can answer a bounded probe. |

Liveness remains successful during a Redis outage. Readiness returns
`503 Service Unavailable` with a stable error body. Neither response
includes Redis URLs or credentials.

## TDD checklist

Write these tests before behavior:

1. Every protocol DTO has a JSON round trip.
2. Unknown enum variants fail with a structured serde error rather than
   silently choosing a default.
3. `TableRevision` serializes as an integer, not a Redis Stream ID.
4. secret wrappers redact their `Debug` output.
5. `/health/live` succeeds with a failing repository double.
6. `/health/ready` returns 503 when its Redis probe fails.
7. the server router can be built with a repository double and no live
   Redis process.
8. a dependency check proves `protocol` does not depend on `cli`,
   `server`, Axum, Tokio, or Redis.

## Implementation route

1. Create `protocol` with serde and thiserror only where necessary.
2. Write identifiers and errors, then DTOs.
3. Create the `server` library around an `AppState` whose dependencies
   are explicit.
4. Put router construction in the library; keep `main` to configuration,
   dependency construction, startup, and shutdown.
5. Add health handlers last.

Do not choose Redis key shapes or table actor messages here. Those are
the learning target of later slices.

## Gates

From `gomoku/`, the implementation slice is complete only when these are
green:

```bash
cargo test -p protocol
cargo test -p server
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Capstone

Start Redis, launch `gomoku-server`, and fetch both health endpoints.
Then stop Redis: liveness stays green while readiness turns into the
stable 503 response. That contrast is the first proof that the shell has
real boundaries.
