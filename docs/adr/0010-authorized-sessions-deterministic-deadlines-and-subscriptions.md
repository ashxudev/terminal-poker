# ADR 0010: Authorized Sessions, Deterministic Deadlines, and Subscriptions

- Status: Accepted
- Date: 2026-08-31
- Deciders: Product owner through Sprint 6 activation; delivery lead for reversible in-process detail

## Context

Sprint 5 put the retry-safe protocol authority behind one bounded worker, but its trusted local methods still accept a caller-selected seat and projection audience. The actor has no authoritative clock and cannot publish ordered player-specific updates. Adding sockets now would expose authorization, timeout, and fan-out semantics before they are executable.

Protocol v1 also identifies the table and revision but omits the hand identifier required by the multiplayer contract. No remote consumer or compatibility promise exists yet, so the missing identity can be corrected before transport selection.

## Decision

1. Add required `hand_id` identity to gameplay commands and validate it before command retention or domain mutation. Accepted events, errors, acknowledgements, and snapshots carry the same explicit table/hand boundary.
2. Keep existing caller-selected actor methods as a trusted local/test compatibility seam. Add remote-safe methods that accept only an opaque server-issued guest-session identifier; the worker derives its bound table, hand, seat, and projection audience.
3. Store bindings only inside the table worker. One session has one role, and one player seat has one owner. A role is either player seat or spectator. Session identifiers are internal keys, are not serializable, and render redacted in diagnostics.
4. A disconnected session retains its binding but cannot submit commands or receive further subscription updates. Reconnect credential issuance and durable recovery remain E8 work.
5. The worker owns monotonic logical time. Accepted transitions schedule a deadline for the next actor. Tick messages emit one warning and, at expiry, select check when legal or fold otherwise.
6. Sprint 6 uses a deterministic 60-tick action window with a warning at 10 remaining ticks. The clock continues across disconnection. These are runtime defaults for evidence, not an irrevocable product policy.
7. Automatic actions use a reserved server-command namespace and the same protocol authority, revision, validation, event, projection, idempotency, and broadcast path as player commands. Remote client IDs cannot claim that namespace.
8. A subscription is a bounded in-process stream keyed by session. The initial snapshot and later accepted-action, deadline-warning, timeout-action, and connection-state updates have one monotonically increasing actor stream sequence.
9. Every subscriber delivery constructs a fresh player or spectator projection from the session role. Rejected commands do not broadcast. A full or closed subscriber is removed rather than blocking the table worker, and public-safe counters record deliveries and drops.
10. Socket framing, TLS, durable credentials, persistence, reconnect replay, lobby/table directories, cross-table routing, supervision, and deployment remain outside this decision.

## Consequences

- Remote-facing callers cannot claim a seat or private audience per request.
- Wrong-table, wrong-hand, cross-seat, spectator, unknown, and disconnected commands fail before poker mutation.
- Time is deterministic under tests and cannot be controlled by a client clock or UI animation.
- Slow subscribers cannot stall table mutation.
- Adding hand identity changes the pre-transport v1 JSON shape; older incomplete shapes fail closed rather than being guessed.
- Bindings, deadlines, and subscriptions disappear on process restart; R-003 remains open.

## Rejected alternatives

- Trust a session ID embedded in the client command envelope: rejected because the gateway/runtime must authenticate and supply session context separately from untrusted JSON.
- Accept an audience argument on remote-safe snapshot or subscription calls: rejected because it permits private-view confused-deputy defects.
- Use wall-clock sleeps in the actor: rejected because tests become slow and nondeterministic and UI/host scheduling can control authoritative outcomes.
- Run a separate timer thread that mutates the hand: rejected because it violates one serialized mutation authority.
- Block the actor while a subscriber buffer is full: rejected because one slow client would deadlock or delay every player.
- Select WebSocket now: rejected because transport is unnecessary to prove authorization, timing, and routing semantics.

## Evidence

- Cross-seat, wrong-table, wrong-hand, spectator, unknown, duplicate-owner, and disconnected negative tests
- Automatic-check and automatic-fold tests with monotonic tick injection and late-command rejection
- Two-subscriber ordering and visibility tests plus bounded slow-consumer removal
- Negative serialization scans for session, credential, deck, and random-state fields
- Continuous Sprint 6 actual-Ratatui trajectory and visually inspected PDF review
