# ADR 0006: Validated seat-command boundary

- State: Accepted
- Date: 2026-08-30
- Owners: Engineering
- Related decision: D-009
- Related story: E1.5b

## Context

The original offline application called a public mutation method directly for both the local player and the bot. That method trusted the caller's seat, turn, phase, action kind, and amount. A future network adapter must treat all controller input as untrusted and must not create a second rules path.

## Decision drivers

- Give every controller one domain entry point.
- Reject invalid intent before any authoritative mutation.
- Return structured errors that network adapters can eventually map to protocol responses.
- Preserve the offline heads-up experience while multiway betting remains deferred.
- Keep controller concerns and player labels outside the domain command.

## Decision

Controllers submit `SeatCommand { seat, action }` to `GameState::apply_command`.

Before mutation, `validate_command` checks:

1. the hand is in an active betting phase;
2. the seat is occupied and eligible to act;
3. the seat is the current actor; and
4. the action kind and amount are legal for the actor's commitment and stack.

Rejected commands return `CommandError` and do not change cards, seats, chips, pot, phase, actor, action history, counters, or result state. The mutation routine is private and reachable only after successful validation.

The terminal's named local-seat adapter and rule-based bot both construct the same `SeatCommand`. UI statistics are updated only after a non-mutating validation pass, and authoritative mutation still occurs exclusively through `apply_command`.

`Call` carries chips added now. `Bet`, `Raise`, and `AllIn` carry the actor's total street commitment. A normal call must match the outstanding amount; a stack-limited call is expressed as `AllIn`. A full-stack total must use `AllIn`, which makes the actor's all-in participation explicit.

This decision establishes controller neutrality, not general multiway betting correctness. Betting-round completion, short-all-in reopening, side pots, and multiway showdown remain E2 work.

## Consequences

### Positive

- Local, bot, and future network controllers cannot bypass turn and amount validation.
- Failed commands are safe to retry or report because they are mutation-free.
- Structured rejection reasons are directly testable.
- The authoritative state transition path is easier to audit.

### Negative

- UI adapters currently validate once before auxiliary statistics and again inside `apply_command`.
- Action amount semantics remain represented by enum convention rather than distinct chip-amount types.
- The current validator intentionally reflects heads-up full-raise rules only.

## Validation

- Focused state tests accept a legal command through `apply_command`.
- Wrong-turn, unoccupied-seat, ineligible-seat, terminal-hand, malformed-call, and malformed-all-in commands are rejected.
- Each rejection test compares a complete authoritative snapshot before and after.
- All active integration tests use the command boundary and preserve existing heads-up behaviour.

## Follow-up

- Add command IDs, expected table revisions, and idempotency at the protocol boundary in E4/E5.
- Generalize available-action calculation and action reopening in E2.
- Separate accepted domain events from command intent for persistence and recovery in ADR 0004.

## Revisit when

Revisit when network commands, multiway betting, timeouts, or recovery introduce metadata beyond seat and poker action.
