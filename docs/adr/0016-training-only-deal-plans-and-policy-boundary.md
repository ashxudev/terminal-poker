# ADR 0016: Training-only deal plans and projection-native policy boundary

- State: Accepted
- Date: 2026-09-01
- Owners: Engineering
- Related decision: AD-005
- Related programme: Poker AI learning curriculum, initial engineering steps 1-6

## Context

ADR 0005 separates entropy-backed production shuffling from deterministic review
seeds. A seed can replay one sampled hand, but mathematical training also needs
exact private-card assignments, public-runout branches, card-removal-aware range
sampling, and duplicate policy evaluation on the same deal.

Adding those capabilities must not allow a client or policy to choose production
cards, inspect an opponent's cards, or read the unused deck. Training also must
reuse `MultiwayHand` and `ProtocolAuthority` rather than creating a second poker
engine.

## Decision drivers

- Preserve the entropy-backed production constructors from ADR 0005.
- Make exact mathematical and duplicate fixtures reproducible.
- Validate all 52 cards before authoritative dealing.
- Keep policies on the existing player-projection and command boundary.
- Keep private decision trajectories separate from public safe history.
- Avoid adding training data to protocol v1 or durable production checkpoints.

## Options considered

### Mutate cards after an ordinary hand is created

Result: Rejected. Post-construction mutation could invalidate uniqueness,
shuffle authority, and showdown invariants.

### Add a public ordered-deck constructor to production table configuration

Result: Rejected. A broadly callable constructor could let an untrusted route
influence cards and would blur the production fairness boundary.

### Add a trusted training plan and crate-private prepared-deck seam

`DealPlanV1` validates a complete card order. The training adapter may construct
it from the existing deterministic shuffle, exact seat assignments plus a public
board, or a branched public runout. Named weighted ranges sample with a separate
RNG after removing blocked cards.

Result: Selected.

## Decision

- Production `MultiwayHand` constructors continue to use `ShuffleSource::production`.
- Review-seeded constructors retain their existing behavior.
- Only the crate-internal, explicitly training-named path accepts a prepared
  `Deck`, after `DealPlanV1` verifies version, 52-card count, and uniqueness.
- `MultiwayHand` still performs all dealing, blinds, action validation, street
  advancement, pot construction, and showdown.
- Policies receive `PolicyObservationV1`, constructed from their player
  `SnapshotEnvelope` and contiguous accepted public `EventEnvelope` values.
- Policies return `PolicyActionV1`; a deterministic legal mapper produces an
  ordinary domain `Action`, which crosses `CommandEnvelope` and
  `ProtocolAuthority` like every other controller action.
- Deal plans, seeds, unused cards, opponent private cards, range labels, and
  policy RNG state never enter policy observations, protocol v1, safe public
  history, or production checkpoints.
- The fast arena is for synchronous training. Registry/runtime/network paths are
  retained for conformance and eventual approved deployment tests.

## Consequences

### Positive

- Exact private deals and public runouts can be independently reviewed.
- Duplicate evaluations can clone a plan while exchanging policy seats.
- Card RNG and policy/range RNG consumption cannot perturb each other.
- The first arena remains authoritative and projection-native.
- Registry conformance compares the fast path with real safe-history
  finalization and rollover.

### Negative

- Trusted training code can hold the complete future deck and must not be used
  as an ordinary logging or deployment surface.
- A complete 52-card plan is larger than a seed-only manifest.
- Multi-hand lifecycle reset and high-throughput batching remain separate work.

### Follow-up

- Add the independently validated mathematical oracle over authorized
  observations and explicit range/response models.
- Add multi-hand lifecycle sessions before training policies on ring dynamics.
- Benchmark a batched arena before selecting a Python tensor bridge.

## Evidence

- Focused training tests cover invalid plans, exact assignments, runout
  branching, range card removal, observation privacy, legal-action acceptance,
  seeded random-policy completion, duplicate replay, and registry conformance.
- The full gate passes 189 unit tests, 14 active integration tests, 7 active
  process tests, strict Clippy, and an isolated optimized all-feature build.
- The optimized `poker-train` smoke emits a versioned terminal safe history.

## Revisit when

Revisit if ordered deals are needed outside trusted local training, public play
adopts auditable commit/reveal shuffling, protocol clients need mid-hand policy
attachment, or durable active-hand replay is introduced.
