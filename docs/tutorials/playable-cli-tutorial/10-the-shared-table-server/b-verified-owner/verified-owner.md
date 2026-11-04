# 10b — The verified owner

## Intent

Create identity before tables. A person requests a passwordless magic
link, follows it within 15 minutes, and receives a bearer session that
slides for 30 days. Only then may that verified owner create a table.

The only mail implementation is `ConsoleMailer`: it prints the link to
the server console. The important result is not email delivery; it is a
clean identity boundary that can later accept an `SmtpMailer` without
changing handlers or table actors.

## The three records

Keep these concepts separate:

| Record | Purpose | Lifetime |
|---|---|---|
| owner account | Stable identity keyed by normalized email identity. | Durable. |
| magic-link challenge | Proves control of the submitted mailbox once. | 15 minutes, single-use. |
| owner session | Authenticates later owner requests. | Sliding 30 days. |

A magic link is not a session, and a session is not a table. Verification
must complete before `POST /v1/tables` is accepted.

## Mailer boundary

Teach the dependency as a behavior, not a concrete sender:

```rust,ignore
trait Mailer {
    async fn send_magic_link(&self, recipient: &str, link: &str)
        -> Result<(), MailError>;
}
```

The exact async trait spelling is an implementation decision for the
pinned toolchain. The contract is fixed:

- handlers depend on `Mailer`, never `ConsoleMailer`;
- `ConsoleMailer` is selected during application assembly;
- its secret-bearing delivery output is a dedicated tutorial mail sink,
  separate from structured application logs;
- application logs redact the normalized email and token query value;
- a future `SmtpMailer` implements the same behavior.

## HTTP contract

| Method | Path | Result |
|---|---|---|
| `POST` | `/v1/auth/magic-links` | Always returns an accepted response after a valid email shape; avoids account enumeration. |
| `GET` | `/v1/auth/verify?token=...` | Consumes one challenge and returns an owner bearer plus owner identity. |

The raw token appears only in the generated link and the verification
request. Store a keyed HMAC or one-way cryptographic digest for lookup;
a 256-bit random token does not need a slow password hash. Ensure access
logging cannot record the raw query value.

A successful authenticated request refreshes the owner-session TTL to
30 days. Failed authentication does not refresh it. After expiry, the
owner requests another magic link and obtains a new session for the same
account and existing tables.

## State transitions

| Current state | Input | Result |
|---|---|---|
| no challenge | valid request | challenge digest stored; link sent |
| pending challenge | correct token before expiry | challenge consumed; account verified; session issued |
| pending challenge | wrong or expired token | generic invalid-link response |
| consumed challenge | same token again | generic invalid-link response |
| active session | authenticated request | session expiry slides |
| expired session | authenticated request | 401; request a new link |

Challenge consumption and session creation are one atomic Redis
operation. A crash cannot consume the link without creating the session,
or create two sessions from one challenge redemption.

## TDD checklist

1. `ConsoleMailer` receives the correct recipient and a redacted-log-safe
   magic link.
2. only a challenge digest is persisted.
3. a valid challenge succeeds exactly once.
4. an expired challenge fails without creating a session.
5. concurrent redemption yields one success.
6. verification creates or reuses the same owner account for the same
   normalized mailbox.
7. authenticated activity slides the session expiry to 30 days.
8. invalid bearers return 401 and never reveal whether an account exists.
9. `POST /v1/tables` rejects an unverified or expired session.
10. structured application logs and `ApiError` bodies contain neither
    raw token nor full email; only the dedicated `ConsoleMailer` sink
    contains the delivery link.

Use an injected clock and token source. Sleeping for 15 minutes is not a
test strategy.

## Error contract

- malformed email shape: `400 Bad Request`;
- invalid, consumed, or expired magic link: `401 Unauthorized` with one
  indistinguishable public error code;
- mailer or Redis unavailable: `503 Service Unavailable`;
- auth rate limit exceeded: `429 Too Many Requests`.

## Gates and capstone

Run the protocol and server tests plus workspace clippy and formatting.
For the capstone, request a link, copy it from the server console, follow
it once, use the returned bearer on an authenticated probe, and show
that following the same link again fails. No table exists yet—and that
is the point of this slice.
