# ADR 0012: Authoritative Rules and Safe Table-Lifecycle Boundary

- State: Accepted
- Date: 2026-08-31
- Owners: Rules and server
- Related: ADR 0001, ADR 0002, ADR 0006, ADR 0007, Sprint 9 E1.7/E3.1/E3.3/E3.4

## Context

The repository now contains a production multiway rules engine, `MultiwayHand`, and the original heads-up `GameState` used by the offline bot-training interface. Leaving both paths described as peers would allow server rules to diverge. The network table also needs join, reservation, sit-out, return, and leave behaviour, but changing the hand's roster while cards or wagers are live would corrupt turn order, pots, privacy, or chip reconciliation.

## Decision

1. `game::multiway::MultiwayHand` is the only server-authoritative poker rules engine for two through nine occupied seats.
2. `game::state::GameState` is a frozen, heads-up-only offline compatibility adapter. It may support the original local bot workflow, but no server, protocol, actor, transport, or network client module may depend on it as authority.
3. Both paths retain shared neutral domain types, blind constants, hand evaluation, shuffle sources, and the validated `SeatCommand` boundary. Cross-path tests cover the supported heads-up opening contract, passive street completion, and chip conservation.
4. `TableLifecycle` owns stable player identity, seat reservations, occupied table seats, independent connection/table/hand participation, pending boundary transitions, and table run state.
5. A reservation is a bounded claim to one vacant seat. It has no cards, chips, action rights, or hand eligibility. A player can occupy or reserve only one seat at a table.
6. `begin_hand` requires at least two eligible occupied seats and emits a value snapshot of the button and seat stacks. That snapshot constructs `MultiwayHand`; it cannot be changed by later roster requests.
7. Sit-out and leave requests made during a hand are queued. A leave request is terminal for that boundary. Join occupancy is rejected while a hand is active. Connection changes remain independent and never remove action or pot eligibility.
8. `complete_hand` accepts exactly one stack for every snapshotted seat, rejects missing, duplicate, extra, or non-conserving reports, reconciles those stacks, resets in-hand participation, and applies pending transitions exactly once.
9. Tables start or resume with at least two eligible players, pause below two, and close only by explicit command when no hand is active.

## Supported divergence map

| Concern | Server authority | Offline compatibility adapter |
|---|---|---|
| Occupancy | 2-9 seats | Exactly 2 seats |
| Betting and pots | Generalized multiway, side pots, reopening | Legacy heads-up behaviour |
| Controller boundary | `SeatCommand` | `SeatCommand` |
| Shuffle | Production entropy; explicit review seed | Production entropy; explicit review seed |
| Lifecycle | `TableLifecycle` snapshot/reconcile boundary | Session-local `start_new_hand` |
| Protocol/network use | Required | Forbidden |
| New rules features | Implement here | Do not add unless needed to preserve offline compatibility |

## Consequences

- The production server now prepares its initial hand through `TableLifecycle`, then hands the immutable snapshot to `MultiwayHand`.
- A heads-up conformance test exposed and corrected a multiway blind-role defect: in heads-up play the button is the small blind and acts first preflop.
- Lifecycle requests cannot strand or rewrite an active hand. Stack reconciliation forms an explicit anti-corruption boundary between hand settlement and the next roster.
- This decision does not yet provide a public lobby, waiting list, durable identity, reconnect credentials, persistence, blind debt, rebuys, tournament movement, or multiple concurrent table routing.

## Rejected alternatives

- Continue treating both engines as equivalent production authorities: rejected because parity would be implicit and unaudited.
- Rewrite the original offline TUI onto `MultiwayHand` during this sprint: rejected because bot/session UX behaviour is broader than the rules-boundary risk and would add migration churn.
- Mutate `MultiwayHand.seats` on disconnect, sit-out, or leave: rejected because it can change actor traversal and pot eligibility after cards and wagers exist.
- Make reservations hand participants: rejected because a claim has neither chips nor confirmed occupancy.
- Close or pause an active hand immediately when eligibility changes: rejected because table lifecycle cannot supersede hand settlement.

## Verification

- `game::rules_boundary` cross-path conformance tests.
- `game::lifecycle` reservation, duplicate identity, immutability, reconciliation, transition, eligibility, and 2-9 occupancy tests.
- `game::lifecycle_review` deterministic one-hand trajectory and Ratatui captures.
- Full Rust gate plus offline and network process regression suites.
