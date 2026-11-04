# 10e — One invitation, one guest

## Intent

Let a verified owner fill the human table's second participant slot
without creating a guest account. The owner receives a 256-bit URL-safe
invitation token; one anonymous guest redeems it once within 24 hours
and receives a bearer scoped to that table alone.

An invitation is capability delivery, not identity proof. Its safety
comes from entropy, narrow scope, short lifetime, and atomic one-time
use.

## Invitation model

The invitation record binds four facts:

- invitation digest;
- `TableId`;
- the table's single guest slot;
- expiry and issued table revision.

Store a keyed HMAC or one-way digest of the random token, never the raw
token. The raw value appears once in the creation response and is shared
outside the server. Do not put it in actor logs, tracing fields, or error
bodies.

The owner bearer is server-wide for that owner's tables. The guest
bearer is different: it authorizes only the redeemed table and guest
participant. Presenting a valid guest bearer to another table is 403;
an invalid or expired bearer is 401.

## HTTP contract

| Method | Path | Principal | Result |
|---|---|---|---|
| `POST` | `/v1/tables/{table_id}/invitations` | owner | reserves or replaces the unfilled guest invitation and returns the raw token once |
| `POST` | `/v1/invitations/redeem` | anonymous token holder | consumes the invitation and returns a table-scoped guest bearer plus snapshot |

These are table mutations. Their request bodies carry `CommandId` and
expected `TableRevision`. A redemption uses the revision issued with the
invitation. If the table changed incompatibly, redemption returns 409
and the owner can issue a new invitation.

Only human tables accept invitations. An AI table has owner plus hosted
AI and returns 422 for invitation creation.

## Atomic redemption

Route redemption through the `TableActor`, then use one Lua operation to:

1. verify the invitation digest and 24-hour expiry;
2. verify table id, guest slot, and expected revision;
3. reject a filled guest slot;
4. create the guest-bearer digest;
5. append the hosted `GuestJoined` transition to the command journal;
6. append the public event envelope;
7. store the idempotent result;
8. consume the invitation;
9. increment revision and refresh table TTL.

If any step fails, none takes effect. In particular, Redis cannot consume
the invitation while leaving the table without its guest.

An ambiguous network failure must still be safely retryable. Keep a
short-lived consumed-invitation record that binds its digest and
`CommandId` to a random 128-bit guest-session id. The guest bearer is the
session id plus a 256-bit HMAC-SHA-256 secret over a domain separator,
table id, and session id, using a configured 256-bit server key. Redis
stores the session record and a digest of the complete bearer, never the
raw bearer. A same-input retry can deterministically reproduce the
committed bearer; a different command id still sees a consumed
invitation. V1 uses one persistent configured key and does not support
online key rotation.

## State behavior

- a forming Swap2 table moves to Opening when the guest joins;
- a forming freestyle table moves to Live and applies the owner's chosen
  color assignment;
- a second guest can never join;
- replacing an unredeemed invitation invalidates its previous digest;
- entering Study mode does not broaden guest authority;
- closing or expiring the table revokes its guest bearer.

## TDD checklist

1. generated tokens contain 256 bits of cryptographic randomness and are
   URL-safe.
2. only a digest is persisted.
3. the creation response reveals the raw token exactly once.
4. owner B cannot create an invitation for owner A's table.
5. AI tables reject invitation creation.
6. correct redemption succeeds once and fills the correct guest slot.
7. two concurrent redemptions produce one guest.
8. expired and replaced invitations fail.
9. wrong-table binding fails without consuming the token.
10. injected Redis failure leaves both invitation and guest slot
    unchanged.
11. retrying the same redemption returns the same committed guest
    session without storing a raw bearer.
12. guest bearer works only on its table.
13. raw invitation and bearer values never appear in logs or errors.

Use deterministic token sources only in tests. Production assembly must
use the operating system cryptographic random source.

## Error contract

- missing/invalid owner bearer: 401;
- valid owner for another table: 403;
- unknown invitation: 404 with a non-enumerating public message;
- expired or consumed invitation: 410;
- revision or idempotency conflict: 409;
- invitation requested for an AI or filled table: 422;
- rate limit exceeded: 429.

## Gates and capstone

Run server tests with the Redis integration suite, then workspace clippy
and formatting. The capstone creates one human table, prints its invite,
redeems it from a second client, and proves both that the guest can fetch
that table and that the same token cannot be redeemed again.
