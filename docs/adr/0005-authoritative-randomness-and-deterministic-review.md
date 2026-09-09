# ADR 0005: Authoritative randomness and deterministic review fixtures

- State: Accepted
- Date: 2026-08-30
- Owners: Engineering
- Related decision: AD-005
- Related story: E1.4

## Context

The original `Deck::shuffle` obtained thread-local randomness internally. That made complete hands difficult to reproduce and hid the future server's fairness boundary inside a value object. Tests could validate deck size but not replay the same deal and runout.

The multiplayer architecture requires the authoritative table, not a client, to own shuffling. Sprint review evidence also requires one traceable hand whose screenshots and state ledger can be reproduced.

## Decision drivers

- Preserve server authority over card order.
- Make complete test and review hands reproducible.
- Keep production and deterministic review construction explicit and difficult to confuse.
- Never expose production random state in protocol messages, logs, screenshots, or reports.
- Keep card and hand rules independent from ambient process randomness.

## Options considered

### Keep ambient thread-local shuffling in `Deck`

Result: Rejected. It obscures authority and prevents deterministic replay.

### Accept a client-provided seed for every hand

Result: Rejected. A client could predict or influence the deck, and seed exchange would become a public protocol concern.

### Inject an authoritative shuffle source

Production construction seeds a cryptographic pseudorandom generator from operating-system-backed entropy. Tests and review fixtures may explicitly construct a deterministic source from a public test seed.

Result: Selected.

## Decision

`Deck` creates and deals cards but does not obtain ambient randomness. It accepts a mutable random generator through `shuffle_with`.

`ShuffleSource` owns the random generator at the authoritative game-state boundary:

- `ShuffleSource::production` seeds `StdRng` from system entropy.
- `ShuffleSource::deterministic_for_review` seeds the same algorithm from an explicit `u64` used only for tests and review fixtures.
- `GameState::new` always uses production construction.
- `GameState::new_seeded_for_review` is explicitly named and documented as non-production.
- One source advances across successive hands rather than resetting the seed per hand.

Clients and controller commands never provide cards, deck order, random bytes, or a production seed.

The deterministic review seed may appear in test evidence and the Sprint 2 review report. Production random state must never be serialized, logged, persisted in ordinary diagnostics, or exposed to any player view. Future recovery design must persist enough authoritative state to resume a hand without reshuffling, but its secure storage format is deferred to ADR 0004.

## Consequences

### Positive

- Identical review seeds reproduce identical deals and runouts.
- The deck is deterministic under an injected generator and easier to test.
- The future server has a clear construction boundary for secure randomness.
- Review screenshots can be regenerated from one known fixture.

### Negative

- `GameState` now owns generator state that future persistence must handle securely.
- `StdRng` output is not a stable cross-version wire format; deterministic fixtures pin the dependency through `Cargo.lock` and are test evidence, not a durable protocol.
- Explicit review construction must never be used for public or adversarial play.

## Validation

- Same-seed deck sources produce identical remaining deck state.
- Different review seeds produce different deck state.
- Same-seed game states deal identical hole cards and preserve identical runout order.
- The normal offline constructor has no seed parameter and selects production construction.

## Follow-up

- Add secure recovery semantics for generator or remaining-deck state in ADR 0004.
- Keep random state out of player projections and ordinary logs.
- Review the chosen CSPRNG and entropy source before public deployment.

## Revisit when

Revisit if the product adopts commit/reveal fairness, externally auditable shuffles, cross-version deterministic replay, hardware randomness, or a regulated environment.
