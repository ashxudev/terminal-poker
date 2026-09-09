# ADR 0002: Module and crate dependency direction

- State: Accepted
- Date: 2026-08-30
- Owners: Engineering
- Related decision: AD-001
- Related story: E0.4b

## Context

The original application is one Rust package whose game, bot, terminal UI, statistics, and binary modules can reference one another. The multiplayer target will eventually need independently testable rules, protocol, server, client, and persistence boundaries. Splitting the repository into several crates immediately would add workspace and API churn before the generalized rules model exists.

The first neutral table-state change therefore needs a dependency direction that works inside the current package and can later be extracted without reversing imports.

## Decision drivers

- Keep poker rules independent from rendering, networking, storage, and wall-clock APIs.
- Prevent controller labels such as human or bot from becoming domain identities.
- Avoid premature public protocol and persistence contracts.
- Preserve the existing offline binary throughout incremental migration.
- Make later workspace extraction mechanical rather than architectural.

## Options considered

### Split into a workspace immediately

Create `poker-core`, protocol, server, client, and storage crates before changing hand state.

Result: Rejected for this sprint. The direction is desirable, but most target boundaries do not yet contain running behaviour and would increase migration scope.

### Keep unrestricted dependencies in one package

Continue allowing game logic to depend on UI, bot, statistics, or future transport types.

Result: Rejected. This would make server authority and deterministic core testing progressively harder.

### Enforce layered modules, then extract stable boundaries

Keep one package for the current migration while enforcing inward-only dependencies. Extract crates after the core command and event surface is demonstrated.

Result: Selected.

## Decision

The repository remains one Rust package during the generalized table-engine migration. Dependencies point inward:

```text
binary
  -> terminal UI / offline orchestration / bot / statistics
       -> pure game domain
            -> identifiers, cards, actions, hand evaluation
```

The game domain must not import terminal UI, bot strategy, statistics persistence, network transport, storage adapters, or wall-clock APIs. Controller identity is an adapter concern: the offline application binds its local person and bot to stable `SeatId` values, while the game domain acts only on seats and stable player identities.

Future protocol messages may translate to and from domain commands but must not become the domain's authoritative representation. Future persistence may store accepted domain transitions but must not own rules.

Create a Rust workspace only after at least one extraction boundary has a concrete independent consumer. The expected first extraction is the pure game domain into `poker-core`, followed by versioned protocol types. Extraction must preserve this dependency direction.

## Consequences

### Positive

- Sprint 1 can remove fixed roles without unrelated workspace churn.
- The offline game remains a real compatibility consumer of the neutral domain.
- Core state stays suitable for deterministic server execution later.
- Crate extraction can follow demonstrated interfaces.

### Negative

- Compiler-enforced crate privacy is deferred.
- Review and tests must enforce module direction in the interim.
- Some offline naming remains in UI and bot adapter code until separate clients exist.

## Evidence

- `game` currently has no dependency on `ui`, `bot`, or `stats`.
- The offline application already orchestrates bot decisions outside the game state.
- Sprint 1 acceptance requires neutral seat ownership while preserving the current binary.

## Follow-up

- Keep neutral table collections and traversal inside `game`.
- Bind offline controllers to seats in the application layer.
- Revisit workspace extraction when versioned commands and private projections are introduced.
- Add an automated dependency-boundary check if module coupling becomes difficult to review.

## Revisit when

Revisit when the first authoritative network server is implemented, when multiple binaries require the domain independently, or when compile-time separation materially reduces privacy or compatibility risk.
