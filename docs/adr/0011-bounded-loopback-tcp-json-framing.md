# ADR 0011: Bounded Loopback TCP and Length-Prefixed JSON

- Status: Accepted
- Date: 2026-08-31
- Deciders: Product owner through Sprint 8 activation; delivery lead for reversible local-alpha transport detail

## Context

Protocol v1, one serialized table authority, opaque guest bindings, bounded subscriptions, and a projection-only client exist in process. Release B now requires the same contract to cross a real byte and process boundary through normal server/client entry points. Sprint 8 excludes TLS, internet exposure, durable credentials, deployment, lobby routing, and process-restart recovery.

The transport must handle partial and coalesced reads, reject oversized or malformed input before authority mutation, preserve ordered server output per connection, avoid adding a second poker protocol, and remain small enough to validate locally without introducing an asynchronous runtime solely for the first loopback slice.

## Decision

1. Use persistent TCP connections bound to an explicitly configured socket address. Sprint 8 defaults to loopback and refuses non-loopback binds or connects.
2. Frame every message as a four-byte unsigned big-endian payload length followed by compact UTF-8 JSON.
3. Bound one payload to 64 KiB and bound undecoded connection buffering to four maximum frames plus headers. Zero-length, oversized, truncated, or malformed frames fail closed.
4. Serialize one tagged client wire enum and one tagged server wire enum. Protocol v1 command, response, update, snapshot, deadline, and error structures remain the only poker contract inside those wire messages.
5. Begin each connection with a versioned connect message containing one opaque local-alpha guest session ID. The server preconfigures session-to-seat bindings; the client does not select its seat or projection audience per request.
6. Use one operating-system thread per accepted connection around the existing bounded table runtime. Each connection serializes its own writes and incrementally decodes reads. The table actor remains the sole poker mutation authority.
7. A connection receives an authorized initial subscription update before gameplay. Command responses are written before queued subscription updates on the submitting connection so pending-command reconciliation is deterministic.
8. EOF or transport failure marks the session disconnected, removes its active subscription, and disables client controls. A later connection with the same local-alpha session reactivates the existing binding and receives a fresh authorized snapshot/update before controls can re-enable.
9. Limit one active TCP connection per guest session and cap accepted connections to the configured table occupancy plus a small administrative margin.
10. Keep transport behind module boundaries so WebSocket/TLS can replace or wrap it before remote/public deployment without changing poker envelopes or `ProjectionClient` semantics.

## Consequences

- Normal server and terminal processes can prove Release B locally without in-process delivery.
- Partial/coalesced read behavior is explicit and unit-testable.
- Compact JSON stays inspectable while the frame prefix removes newline/escaping ambiguity.
- Session names are bearer-like local-alpha handles, not durable reconnect credentials. They are unsuitable for internet exposure.
- One thread per connection is intentionally adequate for a maximum nine-player local table but is not a public scaling decision.
- TCP supplies ordered bytes, while protocol revision, command ID, and stream sequence continue to protect application semantics.
- TLS termination, origin policy, browser compatibility, heartbeats, durable authentication, cross-process table discovery, and public abuse controls remain open.

## Rejected alternatives

- Newline-delimited JSON: rejected because embedded or malformed newlines complicate bounded incremental decoding and recovery.
- Unframed JSON streaming: rejected because object boundary detection becomes parser-dependent and hostile-input handling is harder to prove.
- WebSocket for Sprint 8: deferred because it adds handshake and dependency surface without changing the local alpha authority/reconciliation risk. It remains the leading remote-client candidate.
- UDP: rejected because reliable ordering, retransmission, and connection lifecycle would duplicate transport work.
- An async runtime immediately: deferred because nine bounded local connections do not justify the added concurrency surface yet.
- Client-selected seat on connect: rejected because it would weaken the server-derived authorization boundary established by ADR 0010.

## Evidence required

- Incremental decoder tests for split header, split payload, coalesced frames, zero length, oversize length, malformed JSON, and bounded buffering
- Independent server/client process tests at occupancies two through nine
- Wrong-session, duplicate-active-session, wrong-seat, stale, and malformed remote ingress tests
- Continuous normal-client hand with pending response ordering and final chip reconciliation
- Forced transport loss followed by same-session fresh-snapshot resynchronization
- Existing offline, full Rust, release, and CLI gates
