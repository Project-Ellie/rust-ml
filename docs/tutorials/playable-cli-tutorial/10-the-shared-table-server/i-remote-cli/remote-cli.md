# 10i — An optional remote shell

## Intent

Add an explicit network mode without weakening the local program.
`gomoku play` continues to construct and own a local `SessionCore` with
no server or Redis requirement. `gomoku remote` becomes a protocol
client: REST sends commands, SSE receives committed table updates.

This slice proves the core/shell design from chapter 02. The same command
language now has two shells, but only one of them crosses a network.

## Command-line boundary

The remote entry point is explicit:

```text
gomoku remote --server URL --table TABLE_ID
```

Bearer material comes from a protected environment variable, standard
input prompt, or an operating-system credential helper—not a command
argument that appears in shell history and process listings. The exact
credential helper is outside v1; the tutorial must at least avoid
printing or persisting the bearer accidentally.

Local commands remain:

```text
gomoku play ...
gomoku selfplay ...
```

They do not probe a server, initialize Tokio, or read remote credentials.
A server outage cannot stop local play.

## Reuse and translation

The later remote mode gives `cli` a one-way dependency on `protocol`.
`protocol` remains independent. Do not make remote transport methods part
of `SessionCore`.

Separate the remote shell into:

- command parser: existing textual vocabulary where semantics match;
- protocol mapper: local parsed intent to `WireCommand`;
- REST client: mutation submission and snapshot reads;
- SSE client: committed envelopes, reconnect, reset;
- renderer: existing TUI/line-mode presentation fed from
  `TableSnapshot` and `WireEvent`.

Some local commands stay local:

- `help` renders client help;
- `quit` closes the client;
- display toggles affect only rendering.

The server never journals them.

## Revision loop

The client stores only ephemeral synchronization state:

- latest fully applied `TableRevision`;
- current `TableSnapshot`;
- pending `CommandId`, retained across transport retries;
- last SSE revision.

For each mutation, generate one `CommandId`, send the latest expected
revision, and keep that same id while retrying an ambiguous transport
failure. A 409 stale-revision response triggers snapshot refresh and an
explicit user decision; the client must not silently replay an intent
against a different position.

On an SSE reset, replace the local read model wholesale. Do not attempt
to merge the reset with stale local events.

## Owner and guest experience

The server URL and table id do not imply authority. The bearer decides
whether the client is owner or guest. The snapshot tells the shell:

- human or AI table;
- Forming, Opening, Live, Study, Closed, or Expired;
- participant assignment when colors are known;
- whether the current principal may act;
- current revision and board state.

During Study mode, the guest shell remains live and renders owner
changes, but disables mutation prompts and explains why.

## Failure behavior

- server unreachable: preserve pending command id and offer retry;
- request timed out after send: retry with the same id;
- 401: discard in-memory credential and ask for a new session/invite;
- 409 stale revision: refresh, render, and do not auto-resubmit;
- 410: render the final gone state and exit cleanly;
- SSE disconnected: reconnect with the last fully applied revision;
- malformed protocol data: fail closed and show a version error.

## TDD checklist

1. local `gomoku play` performs no network initialization.
2. remote argument parsing requires server and table.
3. bearer never appears in clap debug output or errors.
4. local Help and Quit issue no HTTP request.
5. one logical retry reuses its `CommandId`.
6. stale revision refreshes without automatic resubmission.
7. SSE reconnect sends the last fully applied revision.
8. reset replaces the local snapshot.
9. owner and guest capabilities render differently.
10. Study mode guest remains an observer.
11. 410 exits with a clear final message.
12. a scripted local transcript remains byte-identical after remote mode
    is added.

Use a fake protocol server for deterministic client tests. The complete
server participates only in the 10j acceptance suite.

## Gates and capstone

Run focused `cli` argument, local transcript, and remote-client tests,
then protocol/server tests and workspace lint/format gates. The capstone
runs local `gomoku play` with the server stopped, then starts two remote
clients on one table and shows the same committed position in both.
