# No-Limit Texas Hold'em Policy Catalogue

- Version: 1.0
- Date: 2026-08-31
- Scope: two-to-nine-seat ring games and tournament tables
- Story: E0.2
- Overall state: Accepted for ring-game beta; tournament-specific movement/button policy remains separate

## Source and interpretation

The normative tournament baseline follows the [2024 Poker TDA Rules, Procedures, and Illustration Addendum](https://www.pokertda.com/view-poker-tda-rules/), particularly its rules for all-in showdowns, showdown order, odd chips, side pots, dead buttons, and reopening a bet.

The TDA rules do not define every online ring-game product choice. Joining, missed blinds, disconnect grace, table visibility, and some button behaviour are therefore explicit product policies rather than being implied by the poker engine.

## State vocabulary

Connection, table participation, and hand participation are independent dimensions.

| Dimension | Values | Meaning |
|---|---|---|
| Seat occupancy | Empty, Occupied | Whether a stable `PlayerId` owns the seat |
| Connection | Connected, Disconnected | Whether the server currently has a live client session |
| Table participation | Waiting, Active, SittingOut, Leaving | Eligibility for future hands |
| Hand participation | NotDealt, Live, Folded, AllIn, Complete | Eligibility and action state in the current hand |
| Blind obligation | Clear, OwesSmallBlind, OwesBigBlind, OwesBoth | Ring-game entry or return requirement |
| Tournament status | Registered, Playing, Eliminated | Tournament-wide lifecycle independent of a table connection |

Definitions:

- `eligible for next hand`: occupied, table participation is Active, stack is positive, and the table policy permits entry despite any blind obligation.
- `eligible to act`: hand participation is Live and the player is neither folded nor all-in.
- `eligible for a pot`: the player contributed to that pot's tier and has not folded.
- `connected`: affects delivery and timeout handling, not pot eligibility.

An implementation must not infer folded, sitting-out, or eliminated state merely from a lost connection.

## P-001: Button, blinds, and action order

State: Accepted ring default; Normative heads-up behaviour; tournament button policy remains separate

### Rule

- Seat order is clockwise over the table's fixed seat indexes.
- For three or more eligible players, the small blind is the first eligible seat clockwise after the button and the big blind is the next eligible seat.
- Preflop action starts at the first eligible-to-act seat clockwise after the big blind.
- Postflop action starts at the first eligible-to-act seat clockwise after the button.
- Heads-up is a required special case: the button posts the small blind, acts first preflop, and acts last postflop.
- Ring-game default: the button moves to the next seat eligible for the next hand.
- Tournament proposed default: dead-button handling, matching the tournament baseline.
- A table changing from three players to heads-up applies the heads-up rule at the next hand boundary; no seat is reassigned mid-hand.

### Examples

| Case | Seats active | Button before hand | Expected |
|---|---|---:|---|
| Three-handed | 0, 1, 2 | 0 | SB 1, BB 2, first preflop 0, first postflop 1 |
| Empty-seat skip | 0, 3, 7 | 0 | SB 3, BB 7, first preflop 0 |
| Heads-up | 2, 6 | 2 | Button/SB 2, BB 6, first preflop 2, first postflop 6 |
| Three to two | 1, 4, 8 then seat 4 leaves | Hand completes with original positions | Next hand recalculates using seats 1 and 8 under heads-up rule |
| All-in skip during hand | Seats 1, 3, 5 live; seat 3 all-in | Action was seat 1 | Next actor is seat 5; seat 3 remains pot-eligible |

### Accepted decision

Ring games use the moving-button default. Tournament dead-button behavior remains outside the ring-game beta and must be fixed before E9.

## P-002: Joining, sitting out, returning, and leaving

State: Accepted ring default and configurable entry mode

### Rule

- Seat ownership changes occur between hands.
- A player joining during a hand waits until the next safe hand boundary.
- Ring default: a new or returning player waits for the natural big blind or posts one live big blind to enter sooner when the table configuration permits.
- A sitting-out player receives no cards and is skipped for button/action eligibility, but retains their seat and stack.
- A request to leave during a hand becomes Leaving; the player remains bound by the current hand until it ends or their hand is folded by the timeout policy.
- Tournament seat assignment and movement override ring-game posting and blind-debt policy; a moved tournament player keeps their exact stack.

### Examples

| Case | Input | Expected |
|---|---|---|
| Join during flop | Empty seat is claimed while a hand is active | Seat is reserved; player is NotDealt until a later hand |
| Sit out between hands | Active player requests sit-out | No cards or blinds next hand; seat and stack remain |
| Sit out in hand | Live player requests sit-out | Request applies after the hand; current action rights remain |
| Leave in hand | Live player requests leave | State becomes Leaving; no chip refund or forced pot removal |
| Tournament move | Controller moves player with 12,450 chips | Destination receives exactly 12,450; player cannot be dealt at source and destination |

### Accepted decision

The wait-or-post-live-big-blind entry policy is accepted for ring games.

## P-003: Missed blinds

State: Accepted ring policy

### Rule

- Blind debt is tracked independently from connection and sit-out state.
- A ring-game player who misses the big blind owes a live big blind before re-entry. A player who owes both blinds posts one live big blind plus one dead small blind; the dead small blind enters the pot but does not count toward the live current bet.
- Blind debt is cleared only by natural posting or an accepted re-entry post.
- Tournament players do not accumulate ring-game blind debt; tournament button and balancing policy governs missed positions.

### Examples

| Case | Input | Expected |
|---|---|---|
| Missed BB | Player sits out as natural BB passes | OwesBigBlind is recorded |
| Return before BB | Player returns two seats before natural BB | Wait, or post under configured entry policy |
| Natural BB return | Player returns when naturally BB | Posts live BB and debt clears |
| Disconnect | Client disconnects but player remains dealt in | No blind debt is inferred solely from connection state |

### Accepted decision

Owing both blinds requires a live big blind and dead small blind before re-entry. Natural posting clears the corresponding obligation.

## P-004: Full raises and short all-ins

State: Normative

### Rule

- The minimum opening bet is the configured big blind unless a smaller all-in is the only possible wager.
- A full raise is at least the size of the last full bet or raise increment.
- A short all-in can increase the amount to call without becoming a full raise.
- A player who has not yet acted retains all legal options when action reaches them, including raising if their stack allows.
- A player who already acted regains the right to raise only when the total additional amount they now face since their last action is at least one full raise.
- Multiple short all-ins may cumulatively reopen action for a previously acting player.
- Short all-ins do not reduce the minimum full-raise increment.

### Examples

All amounts below are total street commitments.

| Case | Sequence | Expected |
|---|---|---|
| One short all-in | A bets 100; B all-in 150; action returns directly to A | A faces 50, so A may fold or call but may not raise |
| Unacted player | A bets 100; B all-in 150; C has not acted | C may fold, call 150, or make a full raise to at least 250 |
| Cumulative reopen | A bets 100; B all-in 125; C calls 125; D all-in 200; action returns to A | A faces 100 more, so action is reopened; minimum raise is to 300 |
| Different prior commitment | In the prior sequence C already committed 125 and action returns at 200 | C faces only 75, so C may fold or call but may not raise |
| Minimum remains full increment | A bets 300; short all-ins move totals through 500, 650, and 800 | An unacted player may raise to at least 1,100; the full increment remains 300 |

### Test obligations

- Track each player's last faced amount or equivalent action-right state.
- Test cumulative reopening per player, not once globally for the table.
- Test that rejected under-raises do not mutate chips, pots, action rights, or revision.

## P-005: Contributions, refunds, main pot, and side pots

State: Normative

### Rule

- Track every player's total hand contribution independently from their current-street contribution.
- Return any uncalled excess before constructing contested pots.
- Construct pot tiers from distinct contribution caps.
- Folded contributions stay in their tiers, but folded players are not eligible to win.
- Resolve and split each pot independently.
- Accepted odd-chip rule, matching the tournament baseline: within each separately resolved pot, award indivisible odd chips to the first tied eligible winner clockwise from the button.

### Examples

| Contributions | Hand state | Expected pots |
|---|---|---|
| A 100, B 100, C 100 | All live | Main 300; A/B/C eligible |
| A 50, B 100, C 200 | All live | Main 150 A/B/C; side 100 B/C; uncalled 100 returned to C |
| A 50 folded, B 100, C 100 | A folded | Main 150 B/C eligible; side 100 B/C eligible |
| A 40, B 100, C 160, D 160 | All live | Main 160 all; side 180 B/C/D; side 120 C/D |
| Pot 101 tied by seats 2 and 7, button seat 0 | Both eligible | 51 to seat 2 and 50 to seat 7 |
| Side pot 51 tied by seats 2 and 7, seat 2 not eligible for that side pot | Seat 7 sole eligible winner | All 51 to seat 7 |

### Accepted decision

Ring games award odd chips clockwise from the button among tied eligible winners for each pot independently.

## P-006: Showdown, revealing, and mucking

State: Normative tournament baseline; Accepted ring default

### Rule

- When one or more players are all-in and all betting is complete, all hands still eligible for any pot are revealed.
- At a non-all-in showdown, the last aggressive player on the final betting street reveals first.
- If the final street checked through, the first live player clockwise from the button reveals first.
- A player may muck rather than reveal at a non-all-in showdown, forfeiting eligibility for unresolved pots.
- If every opponent mucks without revealing, the final live hand wins without being required to reveal.
- A player must reveal both hole cards to claim a contested Hold'em pot, including when playing the board.
- Public hand history contains only cards that were legitimately revealed.

### Examples

| Case | Expected |
|---|---|
| Three-way all-in on turn | All three live hands reveal before board runout |
| River bet and two calls | River aggressor reveals first; callers may reveal or muck in order |
| River checks through | First live seat clockwise from button reveals first |
| All opponents muck | Remaining live hand wins without revealing |
| Player claims board tie with cards concealed | Claim is invalid until both hole cards are revealed |

### Accepted decision

Ring games use the tournament-aligned reveal/muck policy described above.

## P-007: Disconnect grace and action timeout

State: Accepted private-beta product policy

### Rule

- Losing a connection does not immediately change hand participation or pot eligibility.
- Allow a reconnect grace period of up to 60 seconds, bounded by the table's action-clock policy.
- At the authoritative action deadline, automatically check when checking is legal; otherwise fold.
- Never automatically call, bet, or raise for a disconnected human player.
- Tournament level clocks continue during individual disconnections.
- Reconnection restores the player's exact seat and private view; it does not reset accepted actions or duplicate a command.

### Examples

| Case | Expected |
|---|---|
| Disconnect out of turn | Hand remains live; no action occurs until the player's turn or other policy event |
| Disconnect when check is legal | Reconnect until deadline; auto-check at deadline |
| Disconnect facing bet | Reconnect until deadline; auto-fold at deadline |
| Reconnect after command acknowledgement was lost | Snapshot includes accepted result; retry command ID does not apply twice |
| Tournament disconnect | Player-specific grace applies; blind-level clock continues |

### Accepted decision

Private beta uses a constant 60-second grace and check-otherwise-fold behavior. Repeated disconnects do not shorten grace until a later abuse policy explicitly changes it.

## Accepted policy decision packet

The product owner accepted the following choices when activating expanded Sprint 12:

1. Moving button for ring games.
2. Wait for the natural big blind or post one live big blind for entry.
3. If both blinds are owed, post a live big blind plus a dead small blind.
4. Award odd chips to the first tied eligible winner clockwise from the button.
5. Use the tournament-aligned reveal/muck policy for ring games.
6. Use 60-second reconnect grace, then check if legal and otherwise fold.
7. Keep grace constant after repeated disconnects during private beta.

E0.2 is accepted for ring-game beta. Tournament-specific button, movement, and clock interactions remain later E9 decisions.
