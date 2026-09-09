# ADR 0015: Durable Private-Beta Credentials, History, and Drain Boundaries

- State: Accepted
- Date: 2026-09-01
- Owners: Server, protocol, storage, client, and operations
- Related: ADR 0008, ADR 0010, ADR 0011, ADR 0013, ADR 0014, Sprint 13 E10.1a/E10.2a/E10.6a/E10.3b
- Supersedes: ADR 0014 decisions 2, 4, and 8 only where they persisted caller-selected guest routes or used the schema-v1 registry integrity boundary

## Context

The controlled private beta could restore between-hand tables, but its schema-v1 checkpoint retained caller-selected session labels and recoverable private join material. The normal wire handshake treated a label as seat authority, safe histories existed only in memory, and graceful drain was not connected to an operating-system signal. Those choices were acceptable only as a local-alpha seam; copying them into tournament control would widen the trust and recovery weaknesses.

The hardening slice must preserve loopback play, one serialized authority per table, bounded collections, between-hand recovery, and client privacy. It does not add durable accounts, password recovery, public transport, active-hand replay, or encrypted operator storage.

## Decision

1. The server creates a random stable principal for each admitted guest. A caller label remains diagnostic presentation only and never authorizes a table route.
2. A reconnect capability is a 32-byte operating-system-random opaque bearer bound to one stable principal, table, role, and expiry. The server returns it only in the private welcome; ordinary debug output is redacted.
3. The normal client retains the current bearer in an explicitly selected client-side credential file. Bearers are not accepted as command-line arguments and are absent from logs, public projections, lobby data, health, histories, review evidence, and server checkpoints.
4. The server stores only a SHA-256 verifier for a bearer. Successful reconnect rotates the capability after the successor route is ready, revokes the predecessor, and rejects replay, expiry, revocation, and cross-scope use without mutating table authority.
5. Private access codes are reduced to salted SHA-256 verifiers. Schema-v2 registry checkpoints contain the verifier and random stable principals, never plaintext access codes, bearer values, or caller-selected session labels.
6. Disconnect cleanup is bound to the exact authority handle installed by a connection. A retired worker cannot disconnect or remove the successor route after credential rotation.
7. Safe ring histories use a separate format, checksum, atomic file, and 4 MiB document bound. At most 512 terminal spectator histories and accepted public events are retained. Hidden cards, deck/random state, live authority, rejected commands, caller labels, and credential material have no representation.
8. A corrupt or unsupported history document fails closed and is ignored without preventing the independently validated table checkpoint from restoring.
9. An operating-system interrupt stops new admission, allows a bounded drain of at most five seconds, publishes at most one final table checkpoint and safe-history image, and emits one secret-safe completion diagnostic. Recovery retains the documented between-hand RPO.
10. SHA-256 use here supplies one-way verification and corruption detection. It is not encryption, transport authentication, or protection against an attacker who can rewrite server files and process memory.

## Consequences

- A stolen live bearer is still authority until expiry or rotation; local file permissions and the loopback/private-host boundary remain important.
- Server restart can authenticate a client-held bearer without recovering the bearer itself or trusting a caller label.
- The credential handoff and exact-route cleanup add ordering constraints, covered by normal-process replay and successor-route tests.
- Table recovery and history recovery can degrade independently.
- The normal interrupt path has a measurable operational contract and does not rely on a review-only command.
- ADR 0014's between-hand authority/RPO, atomic-publication, whole-document-validation, and hidden-state exclusions remain in force; its route and registry-integrity representation is replaced by schema v2.

## Rejected alternatives

- Persist plaintext access codes or encrypt them with an in-repository key: rejected because both remain recoverable authority in the same failure domain.
- Hash caller labels and keep them as identities: rejected because low-entropy caller choices remain guessable and continue to mix diagnostics with authority.
- Store raw reconnect bearers server-side: rejected because a checkpoint disclosure would immediately authorize seats.
- Rotate before the successor route is ready: rejected because a transient handoff failure can consume the only valid client credential.
- Put histories inside the registry checkpoint: rejected because optional analytical retention should not expand or block authoritative table recovery.
- Treat an in-band shutdown command as signal evidence: rejected because it bypasses the normal operator/process boundary.

## Verification

- Serialization negatives scan registry, history, health, projection, logs, and process arguments for join codes, bearers, caller labels, hidden cards, decks, and random state.
- Unit tests cover redaction, scope, expiry, rotation, revocation, verifier restoration, checksums, retention, and corrupt-history isolation.
- Normal-process tests prove old-token rejection, one-second expiry rejection, client-file reconnect, two-table history growth across restart, and exact-route handoff.
- A real Windows control-break test proves one bounded drain checkpoint, safe completion diagnostics, restart, and history-corruption isolation.
- The optimized two-hour 8-table/32-session soak exercises repeated credential rotation, checkpoints, histories, health, latency, memory, conservation, and zero-alert thresholds.
