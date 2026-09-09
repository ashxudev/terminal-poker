# ADR 0007: Multiway Betting, Pots, and Showdown Semantics

- Status: Accepted
- Date: 2026-08-30
- Decision owners: Product owner and delivery lead
- Accepted by: Product-owner activation of the Sprint 3 goal and acceptance boundary

## Context

Sprint 3 generalizes the authoritative hand engine from a preserved heads-up adapter to deterministic two-to-nine-seat play. Correctness depends on three coupled policies that were previously recorded as recommended defaults: short all-in reopening, tied-pot odd chips, and showdown reveal behaviour.

The engine must also represent response obligations, total contributions, pot eligibility, and awards without relying on terminal-controller roles.

## Decision

1. Every controller submits a `SeatCommand`; legal actions are derived for the authoritative acting `SeatId`.
2. A betting street completes only when every live seat has acted against the current wager and either matched it or cannot act.
3. A full raise reopens action and establishes the next minimum full-raise increment.
4. A short all-in can increase the amount to call without reopening raising. Multiple short all-ins cumulatively reopen raising for a previously acting seat once the increase faced since that seat's last action reaches the last full-raise size.
5. Pot layers are derived from total hand contributions. Folded chips remain in pot amounts, folded seats are ineligible, and a layer funded by only one contributor is returned as unmatched excess.
6. Each main or side pot resolves independently against its own eligible seats.
7. Tied pots are split equally. Odd chips are awarded clockwise from the button among tied eligible winners.
8. All-in showdowns reveal the eligible hands required to resolve every pot. A hand ending by folds does not reveal hidden cards.
9. Chip conservation and card uniqueness are authoritative invariants. Rejected commands do not mutate state.

## Consequences

- Action reopening can be calculated from the current wager, the last full-raise size, and the wager level each seat most recently acted against.
- Street response state is independent for each seat and resets at each street.
- Total hand contribution is distinct from current-street contribution.
- One showdown may produce several winner sets and payout records.
- The existing offline heads-up adapter may remain stable while callers migrate to the neutral multiway engine.
- Broader randomized action-sequence testing remains E2.6; Sprint 3 still requires focused scenario and conservation tests.

## Rejected alternatives

- Treating every all-in increase as a full raise: rejected because it incorrectly reopens action.
- Building a single aggregate pot: rejected because it cannot represent contribution caps or eligibility.
- Awarding tied odd chips by seat number: rejected because it ignores button-relative poker order.
- Reusing UI-visible action state as authority: rejected because presentation is not a rules boundary.
