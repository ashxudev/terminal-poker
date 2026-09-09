# ADR 0001: Server authority and serialized table actor

- State: Accepted
- Date: 2026-08-30
- Owners: Product and Engineering
- Related decisions: D-005, AD-002
- Related story: E0.4a

## Context

The existing game owns the deck, applies player actions, runs the bot, advances streets, and renders results in one local process. A networked table cannot trust clients with deck order, hidden cards, action legality, timers, chip balances, or outcomes.

At the same time, many tables must make progress concurrently without two commands racing within one table. Reconnection and future recovery also require one unambiguous accepted order of table transitions.

## Decision drivers

- Prevent hidden-card and deck-order leakage.
- Reject illegal, stale, duplicate, and out-of-turn commands without mutation.
- Produce one deterministic order of accepted actions per table.
- Allow independent tables to run concurrently.
- Support reconnect snapshots and later durable replay.
- Use the same rules boundary for human and bot controllers.

## Options considered

### Client-authoritative host

One player's client owns the deck and state while peers connect to it.

Benefits:

- Minimal central infrastructure
- Fast local prototype

Costs and risks:

- Host can inspect or alter hidden state.
- Host loss terminates or complicates recovery.
- NAT, availability, migration, and dispute handling move into the client.
- A compromised host can forge chips and outcomes.

Result: Rejected.

### Deterministic peer replication

Every client runs the rules engine and reaches consensus on actions and randomness.

Benefits:

- No single gameplay server
- Replicated public state

Costs and risks:

- Secure dealing and hidden information require a substantially more complex cryptographic protocol.
- Disconnect and membership changes complicate consensus.
- Recovery and tournament table moves become distributed transactions.
- Complexity is disproportionate for the play-money product.

Result: Rejected.

### Authoritative serialized table actor

The server owns complete state. Each active table has exactly one logical actor that processes one mailbox command at a time and emits ordered domain events and player-specific views.

Benefits:

- Simple authority and privacy boundary
- Deterministic command ordering
- Natural isolation and concurrency across tables
- Clear idempotency, revision, timer, and recovery model

Costs and risks:

- Server availability is required for play.
- Actor placement and recovery must preserve singular ownership.
- Public scale eventually requires routing and sharding.

Result: Selected.

## Decision

The complete authoritative `TableState`, including deck order and all hidden cards, is owned by one logical server-side table actor at a time.

Clients and bots submit commands. They do not mutate `TableState`, submit replacement state, select cards, advance streets, award pots, or control authoritative time.

For every command, the table authority performs this conceptual sequence:

1. Receive a bounded command through the table mailbox.
2. Verify session, table, seat, hand, command ID, and expected revision.
3. Return the recorded result for a duplicate command ID.
4. Validate turn, action rights, and amount against current state.
5. Apply one deterministic domain transition if valid.
6. Increment the table revision only for an accepted transition.
7. Emit ordered domain events.
8. Produce explicit player-specific projections for subscribed recipients.

Rejected commands do not change chips, cards, pots, action rights, timers, history, or revision.

Authoritative action deadlines enter the actor as server-generated messages. Client clocks and animation callbacks are informational only.

Bot controllers receive an authorized bot view and submit ordinary commands through the same validation path as human controllers. They do not receive another seat's hidden cards.

Different table actors may run concurrently. One table actor processes state-changing messages serially. Runtime placement, durable event ordering, snapshots, and actor migration will be specified by later ADRs without changing this authority boundary.

## Privacy boundary

Internal table state and network views are different types.

- Internal state may contain every hole card, deck order, and server-only metadata.
- A player projection contains that player's cards plus public state.
- A spectator projection contains public state only, subject to any configured delay.
- The internal state type must not implement or be accepted by an ordinary client-response path.
- Ordinary logs must identify table, hand, command, and revision without logging hidden cards or credentials.

## Failure behaviour

- Losing a client connection does not transfer authority.
- Losing a table runtime pauses progress until that actor is recovered or the table is closed by policy.
- A recovered table must resume at one accepted revision without reshuffling or reapplying command IDs.
- Two runtimes must never both believe they are the current mutation authority for one table.

The precise durability and lease mechanism remains a later decision.

## Consequences

### Positive

- Rules can remain deterministic and transport-independent.
- Privacy can be tested at one explicit projection boundary.
- Table concurrency does not introduce within-table data races.
- Reconnect and recovery have a single source of truth.
- Bots and humans share action validation.

### Negative

- Server infrastructure becomes mandatory for network play.
- Actor supervision, routing, persistence, and split-brain prevention require dedicated work.
- A single table's throughput is intentionally serialized.

### Follow-up

- Define crate dependency direction without allowing protocol types into core rules.
- Define versioned command and view schemas.
- Define durable snapshots, event ordering, and command-result retention.
- Define actor leases and routing before horizontal deployment.
- Add tests proving rejected and duplicate commands are mutation-free.

## Evidence

- Multiplayer requirements require server authority and per-player views.
- Sprint 0 rules policy separates complete state from participation and connection status.
- E1.1a introduces neutral identifiers without coupling them to transport.

## Revisit when

Reconsider only if the product adopts cryptographically decentralized dealing, offline peer-to-peer play, or another requirement that intentionally removes central server authority.
