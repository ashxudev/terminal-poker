# ADR 0009: Bounded Idempotency Ledger and Table Mailbox

- Status: Accepted
- Date: 2026-08-31
- Deciders: Product owner through Sprint 5 activation; delivery lead for reversible runtime detail

## Context

Protocol v1 carries command IDs and expected revisions, but Sprint 4 deliberately did not retain results. A network retry would therefore arrive stale after the original command succeeded, and command-ID reuse could be ambiguous. The poker authority is also still called synchronously by local code rather than owned by the single serialized table actor selected in ADR 0001.

## Decision

1. Each active hand authority retains a bounded in-memory map from validated command ID to the command fingerprint and original public outcome.
2. The fingerprint is the expected revision plus controller-intent payload after protocol version, ID syntax, and table identity are validated.
3. An exact retry returns the original accepted or rejected outcome. Its acknowledgement marks delivery as replayed; state and revision do not change.
4. Reusing a retained ID with a different fingerprint returns a stable `command_id_conflict` rejection at the current revision.
5. Valid-scope stale and domain-rejected commands are retained so their rejection remains stable. Unsupported-version, invalid-ID, wrong-table, and decode failures are not retained because they do not establish a valid table command identity.
6. The ledger never evicts within the hand. If its fixed capacity is exhausted, a new command fails closed with `command_ledger_full`. Durable retention and hand-boundary rollover belong to later persistence/table-lifecycle work.
7. JSON command ingress is bounded before deserialization. Parser detail and hostile input are not echoed in public errors. Unknown or malformed v1 shapes fail closed.
8. The first table actor is an in-process bounded mailbox with exactly one worker owning `ProtocolAuthority`. Handles submit commands, request audience-specific snapshots, and request public-safe counters; they cannot access or mutate the hand directly.
9. Runtime placement, socket transport, sessions, timers, persistence, supervision, recovery, and multi-table routing remain later decisions.

## Consequences

- Accepted and rejected retries are deterministic before a network transport exists.
- A bounded no-eviction ledger preserves at-most-once mutation within one active hand.
- One worker makes within-table races impossible by construction while allowing concurrent producers.
- The mailbox and ledger provide a thin server seam without coupling core rules to threads or transport.
- Restart recovery does not yet preserve idempotency; this remains an explicit R-003 exposure.

## Rejected alternatives

- Check expected revision before the retry ledger: rejected because a successful retry would be reported stale instead of replaying its original result.
- Evict oldest IDs at capacity: rejected because a late retry could then apply twice.
- Treat any matching ID as an exact retry: rejected because conflicting intent could receive an unrelated result.
- Share the authority behind a mutex across callers: rejected because it obscures mailbox ordering and permits future code to bypass the actor boundary.
- Add WebSocket/Tokio transport now: rejected as outside Sprint 5 and unnecessary to prove singular authority.

## Evidence

- Accepted and rejected exact-retry tests, conflicting-ID tests, and ledger-capacity tests
- Bounded decode matrix covering malformed and incompatible input
- Concurrent producer tests proving one accepted mutation and replay/stale outcomes without races
- Audience-specific actor response and negative privacy tests
- Continuous Sprint 5 actual-Ratatui trajectory and visually inspected PDF review
