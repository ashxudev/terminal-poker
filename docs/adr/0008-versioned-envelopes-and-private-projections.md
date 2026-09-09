# ADR 0008: Versioned Envelopes and Private Projections

- Status: Accepted
- Date: 2026-08-30
- Deciders: Product owner through Sprint 4 activation; delivery lead for reversible schema detail

## Context

The multiway engine now owns complete cards, stacks, betting state, pots, and awards. Network consumers need stable messages without gaining the ability to replace authoritative state or observe another player's hidden information. Transport, authentication, persistence, retry deduplication, and server execution are not yet implemented.

## Decision

1. Define a transport-neutral, Serde-compatible protocol version 1 with distinct command, accepted-event, snapshot, and error envelopes.
2. Every envelope carries a protocol version and table identity. Commands also carry a validated command ID and expected table revision. Events echo the command ID and carry the resulting revision. Snapshots carry the revision from which they were projected.
3. A local protocol authority validates version, identity, and expected revision before converting controller intent into the existing authoritative `SeatCommand`. Only a successful domain mutation increments revision.
4. Command IDs are validated and carried for future idempotency, but Sprint 4 does not cache or deduplicate retries; E4.3 owns that behavior.
5. Protocol errors use stable public codes and bounded context. They do not serialize internal errors, cards, deck state, or replacement authoritative state.
6. Construct a new projection for each audience. A player receives their own hole cards and, when acting, their legal actions. Spectators receive no private cards. Any audience receives another seat's cards only when that seat is in the authoritative public reveal set.
7. Never derive a client payload by serializing `MultiwayHand`, `Deck`, a shuffle source, or internal random state.
8. JSON is the acceptance encoding for schema tests and review evidence. Socket transport remains undecided; adopting WebSocket or another transport requires a later ADR.

## Consequences

- Privacy becomes an explicit construction boundary with negative serialized-output tests.
- Stale or misrouted commands fail before poker mutation and revision advancement.
- Schema version rejection is deterministic before a transport exists.
- Future E4.3 can add command-ID deduplication without changing the command envelope's identity fields.
- Future E4.4 must add broader compatibility, malformed-message, and size-limit campaigns before public network ingress.
- Authentication and session-to-seat authorization remain required before a remote client can select a player audience.

## Rejected alternatives

- Serialize complete `MultiwayHand` and redact fields afterward: rejected because newly added private fields could leak by default.
- Reuse one shared snapshot for every recipient: rejected because player-private cards and legal actions differ by audience.
- Omit version or revision until sockets exist: rejected because compatibility and stale-command semantics would become transport-coupled.
- Implement retry deduplication in this sprint: rejected as a separate E4.3 state-management and recovery concern.

## Evidence

- Exact envelope serialization and unsupported-version tests
- Accepted command increments revision once; stale, wrong-table, invalid-ID, and domain-rejected commands do not
- Player-versus-player and spectator projection comparisons
- Negative JSON scans for opponent cards and sensitive field names
- Continuous Sprint 4 Ratatui trajectory and visually inspected PDF review
