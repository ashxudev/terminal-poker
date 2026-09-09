# Neutral table domain model

Sprint 1 replaced fixed human/bot game fields with stable, seat-indexed state while retaining the offline heads-up application.

## Dependency direction

The current package follows [ADR 0002](../adr/0002-module-and-crate-dependency-direction.md):

```text
binary and adapters
  -> terminal UI, bot, statistics
       -> game domain
            -> seat IDs, cards, actions, hand evaluation
```

The `game` module does not depend on controller, rendering, persistence, transport, or wall-clock types.

## State ownership

`TableSeats` owns the physical table capacity and an optional `SeatState` at each `SeatId`. An occupied seat owns:

- Stable `PlayerId`
- Hole cards
- Stack
- Current-street contribution
- Connection state
- Table participation
- Hand participation

The collection rejects seats outside the configured capacity, occupied-seat replacement, and duplicate player identities. It is deliberately not a client-view or persistence type.

## Independent eligibility

- Next hand: positive stack and active table participation
- Action: live hand participation and positive stack
- Pot: live or all-in hand participation

Connection is independent. A disconnected player remains hand-eligible until a future authoritative timeout policy changes their hand state.

## Position calculation

`TableSeats::positions` calculates button, small blind, big blind, first preflop actor, and first postflop actor for two through nine eligible seats. Heads-up uses the standard special case: the button posts the small blind and acts first preflop.

## Offline compatibility boundary

The terminal application binds its local controller to seat 0 and its rule-based bot to seat 1. These names remain in adapter and presentation code only. `GameState`, action ownership, aggressors, positions, showdown results, and per-seat session statistics use `SeatId`.

`GameState::new` still constructs exactly two occupied seats because multiway betting-round completion and side pots are later work. Multiway-capable storage and traversal must not be interpreted as a completed multiway poker engine.
