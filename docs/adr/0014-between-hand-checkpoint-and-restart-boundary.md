# ADR 0014: Between-Hand Checkpoint and Restart Boundary

- State: Accepted
- Date: 2026-08-31
- Owners: Server, lifecycle, and operations
- Related: ADR 0001, ADR 0008, ADR 0010, ADR 0012, ADR 0013, Sprint 11 E7.7/E8.3a/E8.4a/E11.1c

## Context

The Sprint 10 registry was process-local and each table stopped after one hand. Repeated authority now creates a safe instant after settlement and before the next deal. Recovery needs to preserve the lobby, seated stacks, progression, private guest routing, and monotonic identity allocation without serializing live poker authority or replaying a terminal award.

An active hand contains hidden cards, a deck, pots, projections, deadlines, subscriptions, command deduplication records, and socket/runtime state. Persisting that graph would couple storage to volatile implementation details and create privacy and exactly-once recovery risks. Active-hand crash recovery is not part of this release slice.

## Decision

1. Every table retains a durable image captured only at a reconciled between-hand boundary. A process crash may discard the active hand and resume from that boundary; it never resumes midway through a deal.
2. The version-1 registry checkpoint is an explicit allowlist: registry capacity/revision; next table, player, and hand counters; table ID/configuration/seed; seated player ID, seat, stack, table participation; last button and next hand number; and private guest-to-table/player/seat routes.
3. Checkpoints have no representation for decks, cards, pots, awards, projections, commands, deduplication ledgers, timers, sockets, subscribers, actor handles, or runtimes.
4. The payload is deterministically serialized and protected by a labelled FNV-1a 64-bit integrity checksum. This is corruption detection, not authentication or encryption.
5. The complete document is bounded to 1 MiB and uses format label `terminal-poker-registry` with schema version 1. Unsupported versions and unknown/inconsistent identities fail closed.
6. Publication writes and flushes a sibling temporary file before one filesystem replacement. Windows uses `ReplaceFileW` for existing targets; other supported platforms use same-filesystem rename replacement. A failed publication leaves the prior checkpoint authoritative and removes the temporary file when possible.
7. Restore reads at most the configured bound, validates format/version/checksum and the entire identity/configuration/lifecycle graph into a candidate registry, and only then constructs new per-table runtimes.
8. Restored network connections start disconnected. Clients must reconnect using their private guest identity and receive a fresh projection from a newly allocated hand authority.
9. Restored counters must be strictly greater than every restored identity. New hands receive fresh monotonic IDs and empty revision, award, deadline, subscription, and command-ledger state.
10. Checkpoint storage remains local plaintext. Operators must protect the file as private server data. Encryption, authenticated storage, replication, active-hand recovery, and distributed consensus remain excluded.

## Consequences

- A normal server restart can recover multiple table rosters and stacks without retaining or replaying a completed hand.
- Recovery point objective is one between-hand boundary; actions in a later incomplete hand can be lost by design.
- The format is deliberately smaller and more stable than internal Rust structs.
- Guest session identifiers are durable private routing data and must never enter public lobby output or review screenshots.
- FNV integrity catches accidental damage but does not prevent malicious modification by an attacker who can rewrite the file and checksum.

## Rejected alternatives

- Serialize `ProtocolAuthority` or actor state: rejected because it persists hidden and volatile live-hand state and makes exactly-once award recovery unsafe.
- Persist terminal snapshots and replay events: rejected because projections are audience-specific and replay can duplicate awards.
- Save only public lobby summaries: rejected because stacks, player ownership, button progression, and monotonic counters would be lost.
- Write directly to the target file: rejected because a crash can publish a partial document.
- Resume an interrupted hand from its opening boundary while claiming active-hand recovery: rejected because rollback semantics are intentionally explicit in this slice.

## Verification

- Allowlist serialization test asserts sensitive live-authority field names are absent.
- Size, checksum, schema, replacement, and temporary-file cleanup tests.
- Whole-document restore tests for corruption, truncation, unsupported version, inconsistent identities, and counter regression.
- Two-table normal-process save/restart/list/reconnect/complete journey with monotonic new hand IDs, chip conservation, isolation, and no prior award.
- Production Ratatui restart trajectory and page-by-page PDF inspection.
