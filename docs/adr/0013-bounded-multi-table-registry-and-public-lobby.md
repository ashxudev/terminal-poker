# ADR 0013: Bounded Multi-Table Registry and Public Lobby Boundary

- State: Accepted
- Date: 2026-08-31
- Owners: Server and protocol
- Related: ADR 0001, ADR 0008, ADR 0010, ADR 0011, ADR 0012, Sprint 10 E7.4/E7.1/E6.4c/E11.1b

## Context

The local network alpha proved one serialized table authority over a bounded loopback transport. Sprint 9 added a safe between-hand lifecycle, but the production server still constructed one fixed table and pre-bound every seat. Supporting many tables introduces a second routing dimension: table identity must select exactly one authority before untrusted commands, sessions, or projections reach poker state. A public lobby also needs useful discovery metadata without serializing internal hands, sessions, actors, command ledgers, or random state.

The first multi-table slice remains local and ephemeral. It must establish isolation and a real create/list/join path without prematurely choosing durable identity, distributed actor placement, public TLS topology, private join-code policy, waiting lists, or recovery storage.

## Decision

1. A bounded in-memory `TableRegistry` is the sole local process owner and locator for many table runtimes. Its default capacity is 16 tables and its accepted configuration range is 1 through 64.
2. The server assigns monotonically increasing `TableId`, `HandId`, and internal `PlayerId` values. Clients may request a public table or seat but cannot submit replacement identities or authoritative state.
3. Each running registry entry owns exactly one `AuthorizedTableRuntime`, which continues to own exactly one serialized `TableActor` and `ProtocolAuthority`. The registry never mutates poker-hand state.
4. A private server-side guest binding selects one table, hand, seat, and actor handle. Cross-table commands are rejected before actor ingress. The existing authorized runtime performs a second table/hand/seat check.
5. One guest session may occupy one registry table in this slice. This is a session-routing invariant, not a final decision about one durable user playing multiple tables.
6. Public lobby contracts are separate versioned envelopes. Public summaries use an explicit allowlist: table identity, safe name, seat capacity, starting stack, fixed blinds, aggregate occupied/reserved counts, lifecycle status, and join availability.
7. Lobby lists are deterministically ordered and bounded by registry capacity. Filters only narrow that set.
8. Table creation, join, and routing rejections do not advance lobby revision, consume player identity, evict entries, or mutate another table.
9. A table begins its first runtime when at least two eligible occupied seats exist. Additional between-hand joins, waiting-list promotion, and later-hand runtime rollover remain separate lifecycle work.
10. Only an empty inactive table may be removed in Sprint 10. A running authority cannot be implicitly evicted or torn down.
11. Protocol v1 table messages remain intact. New lobby messages are additive at the bounded wire layer; a connected socket transitions from lobby mode to exactly one table mode after a ready join.
12. The initial implementation remains loopback-only, ephemeral, and process-local. No distributed registry, persistence, TLS, internet exposure, or recovery claim is made.

## Consequences

- Normal `poker-server` and `poker-client` processes can create, list, inspect, join, and run multiple independent public tables.
- Existing single-table clients and process tests remain compatible because the legacy server mode is preserved.
- Public metadata cannot reuse player or spectator hand projections, reducing accidental hidden-state exposure.
- Registry capacity and deterministic ordering make public response size testable and bounded.
- Process failure still loses lobby, sessions, and active tables. Release C recovery and operational gates remain open.
- Private/unlisted tables and join codes remain unresolved and unimplemented.

## Rejected alternatives

- One global actor for every table: rejected because an overloaded or defective table would serialize unrelated games and weaken isolation.
- Caller-provided actor handles or table authority: rejected because clients could bypass registry ownership and target another table.
- Serialize `TableLifecycle`, `ProtocolAuthority`, or `TableProjection` into the lobby: rejected because each contains internal or audience-specific state beyond public discovery needs.
- Unbounded map and list results: rejected because table creation or discovery could exhaust memory or transport buffers.
- Replace the proven single-table server immediately: rejected because additive multi-table mode preserves the Release B regression seam while the new boundary earns evidence.
- Introduce persistence or distributed coordination in the same slice: rejected because it would combine routing correctness with a separate recovery and topology decision.

## Verification

- Registry capacity, ordering, filtering, stable identity, mutation-free rejection, safe-removal, and global session-binding tests.
- Public-summary serialization allowlist and sensitive-field absence tests.
- Two in-memory running tables with cross-table command rejection and independent revision/chip snapshots.
- Independent-process create/list/join journey with two tables, four clients, reconnect, terminal outcomes, and per-table chip conservation.
- Existing 2-9 single-table network process matrix and offline/full quality gates.
- Production Ratatui lobby and one-table hand trajectory in the visually inspected Sprint 10 PDF.
