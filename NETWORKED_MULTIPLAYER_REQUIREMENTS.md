# Networked Multiplayer Poker Requirements

The player-facing application shell, menu journeys, installed-command contract,
Practice, Host, Join, and Study requirements are defined in
[Sneaky Blinders Player Experience Requirements](SNEAKYBLINDERS_PLAYER_EXPERIENCE_REQUIREMENTS.md).

## Product scope

The target is a server-authoritative, networked No-Limit Texas Hold'em platform with:

- Two to nine players per table
- Many tables operating concurrently
- Private and discoverable play-money tables
- The existing terminal UI as the initial client
- The existing bot available for offline play and optionally for filling configured seats
- An architecture capable of supporting multi-table tournaments

All chips are play-money and have no monetary value. Real-money wagering is explicitly out of scope.

## Important scope distinction

“Multi-table” can describe two related but different products.

### Multiple concurrent ring-game tables

The server hosts many independent tables. Players browse, create, join, leave, and switch between them. Each table has its own blinds, stacks, seats, hands, and lifecycle.

### Multi-table tournaments

Players enter one tournament spanning several tables. A tournament controller assigns seats, increases blind levels, balances and breaks tables, moves players, tracks eliminations, and determines final standings.

### Functional single-table tournament target

The current delivery target is a multiplayer tournament contained within one
authoritative table for two through nine entrants. It includes configuration,
fixed-start registration, one unique seat and starting stack per entrant,
scheduled blinds/antes/breaks, reconnect, exact-once elimination, deterministic
standings, one winner, and between-hand recovery through the installed product.

It does not include balancing, player movement, table breaking, final-table
consolidation, or hand-for-hand coordination. Those remain a separately named
multi-table tournament expansion. This target narrows the milestone, not the
architecture: many independent table authorities and later tournament
coordination remain supported.

The table engine and server must support both, but the recommended delivery sequence is:

1. Build the generic two-to-nine-seat table engine.
2. Ship multiple concurrent ring-game tables.
3. Add tournament orchestration on top of the proven table engine.

If multi-table tournaments are required in the first public release, the tournament requirements in this document become MVP rather than a later phase.

## Recommended initial rules and platform defaults

- No-Limit Texas Hold'em
- Two to nine occupied seats per table
- Nine physical seats at every full-ring table
- Server-authoritative game state and randomness
- Dedicated server deployment
- Guest display names initially
- Listed open games and listed password-protected games; legacy hidden games retained
- Configurable starting stack and blinds
- 45-second action timer
- 60-second reconnection grace period
- Table-stakes rules
- No rake
- Existing offline bot mode retained
- One active table per terminal client initially

## Player journeys

### Ring game

1. Connect to the server and choose a display name.
2. Browse all listed games on the remembered dedicated server; enter a password only for a protected game.
3. Create a new table or join an available seat.
4. Buy in using play-money chips within the table limits.
5. Mark ready and wait for the minimum player count.
6. Join at a safe hand boundary and post any required blind.
7. Play until leaving, sitting out, disconnecting, or exhausting the stack.
8. Rebuy or leave according to the table configuration.

### Tournament

1. Register before the registration deadline.
2. Receive a table and seat assignment.
3. Begin with the tournament starting stack.
4. Play through scheduled blind and ante levels.
5. Move between tables when balancing or table breaking requires it.
6. Continue until eliminated or declared the winner.
7. View final placement and results.

## Table model

Each table requires stable identifiers and explicit state for:

- Table ID and display name
- New game access: open or password protected, both listed. Legacy unlisted/private visibility stays hidden.
- Game type and rules version
- Maximum seats from two to nine
- Minimum players required to deal
- Seat assignments
- Player connection and participation status
- Dealer button position
- Small blind, big blind, and optional ante
- Buy-in limits or tournament stack rules
- Current hand number
- Current street
- Community cards
- Main and side pots
- Current bets and committed chips
- Player action history
- Action deadline
- Table lifecycle state
- Tournament ID when controlled by a tournament

A seat should have independent statuses rather than one overloaded enum. The server must be able to represent whether a player is:

- Connected or disconnected
- Seated or unseated
- Ready or waiting
- Active, sitting out, or leaving after the hand
- Participating in the current hand
- Folded or still eligible for a pot
- All-in
- Owing a blind
- Eliminated from a tournament

## Nine-handed poker rules

The existing heads-up state machine must be replaced with a general seat-based engine.

### Turn order

- The dealer button advances to the next eligible seat under the configured button policy.
- With three or more players, action begins left of the big blind preflop and left of the button postflop.
- Heads-up remains a supported special case: the button posts the small blind and acts first preflop.
- Folded, all-in, empty, sitting-out, and otherwise ineligible seats are skipped.
- A betting round completes only after every eligible player has responded to the latest full bet or raise.

### Betting

- Check, call, bet, raise, fold, and all-in must be validated server-side.
- The engine must track the amount each player has committed on the current street and for the entire hand.
- Minimum raises must follow the size of the last full raise.
- A short all-in may increase the amount to call without reopening raising for players who have already acted, according to the selected ruleset.
- The server must calculate available actions independently for every acting seat.
- No client-supplied amount may cause an invalid chip balance or integer overflow.

### Pots and all-ins

- Build a main pot and any number of side pots from total player contributions.
- Track which players are eligible to win each pot.
- Folded players' contributions remain in pots but they are ineligible to win.
- Resolve each pot independently at showdown.
- Split tied pots correctly.
- Assign odd chips using one documented rule, recommended clockwise from the button among tied eligible winners.
- Preserve the invariant that all chips are accounted for after every state transition.

### Seating and blinds

The product must explicitly choose and implement policies for:

- Moving-button versus dead-button handling
- Joining immediately, waiting for the big blind, or posting a dead blind
- Missed-blind debt after sitting out
- Players leaving during a hand
- A table falling from three players to heads-up
- A table gaining players between hands
- Tournament seat moves near the blinds

These rules must be configuration or domain policy, not scattered UI conditions.

### Showdown

Sprint 17's installed tournament-style procedure is specified in
[ADR 0017](docs/adr/0017-ordered-tournament-showdown.md). It includes ordered
automatic show/muck with a per-hand show override, mandatory all-in exposure
before runout, private histories, and server-controlled progression. Explicit
manual mucking of potentially winning hands and discretionary requests to see
another caller's hand are outside this procedure.

- Determine whether each remaining player's cards must be shown or may be mucked.
- Reveal only cards required by the configured showdown policy.
- Evaluate every eligible hand once and use the result across all relevant pots.
- Award pots in deterministic order.
- Preserve a complete public hand history without exposing legitimately mucked cards.

## Table lifecycle

```text
Creating -> Waiting -> Starting -> InHand -> BetweenHands
                  ^                    |          |
                  |                    v          v
                  +---------------- Paused     Closing -> Closed
```

Requirements:

- A hand starts only when the minimum number of eligible players is present.
- Seat changes occur only at safe boundaries unless a tournament move requires otherwise.
- A player requesting to leave during a hand remains until their hand is complete or is folded by policy.
- A table pauses or returns to waiting when too few eligible players remain.
- Empty and inactive tables expire automatically.
- Table configuration becomes immutable once the first hand begins, except for explicitly dynamic tournament settings.
- Each table processes commands serially so two actions cannot mutate one hand concurrently.

## Lobby and table management

The lobby must support:

- Creating listed open and password-protected tables (new hidden-game UI deferred)
- Listing available public tables
- Filtering by stakes, seat count, occupancy, and status
- Joining by table ID or short code
- Selecting an open seat or accepting automatic seating
- Table capacity enforcement
- Waiting lists for full tables
- Spectator access only if explicitly enabled
- Table ownership and closure policy
- Clear display of blinds, buy-in, seats, average pot, and players waiting
- Pagination or subscription updates so the full lobby is not resent on every change

The server must prevent one player identity or reconnect token from occupying multiple seats at the same table.

## Tournament controller

Tournament orchestration sits above individual tables and must not be embedded inside table betting logic.

The functional target controls exactly one table. Every configuration and
lifecycle rule below that does not inherently require multiple tables applies to
that target. Cross-table operations are explicitly deferred to the multi-table
expansion.

### Tournament configuration

The first Host tournament journey is configuration-led. The host controls a
bounded pre-start structure, not authoritative live poker state. A draft may be
edited until registration/configuration lock. Start creates one validated,
versioned configuration snapshot; after that, only explicitly dynamic schedule
progression may change it. The host cannot directly alter cards, pots, current
bets, stacks, action rights, eliminations, or awards.

- Tournament ID and name
- Registration opening and closing times
- Start time
- Minimum and maximum entrants
- Starting stack
- Blind and ante schedule
- Level duration
- Scheduled breaks
- Table size from two to nine
- Late-registration policy
- Re-entry or rebuy policy
- Payout or result structure using play-money or non-monetary standings
- Heads-up final-table rules

The installed setup surface must expose, validate, preview, and confirm at
least: entrant capacity/table size, starting stack, starting small/big blind and
ante, ordered blind/ante levels, duration per level, scheduled breaks, and the
play-money payout/result structure. Invalid combinations fail before
registration opens. Payout percentages or units must reconcile to the configured
pool, and rounding/odd units must be deterministic and visible. Nothing in this
contract creates monetary value, a transferable prize, a payment, or a
withdrawal right.

### Tournament lifecycle

```text
Scheduled -> Registering -> Seating -> Running -> HandForHand -> Completed
                   |                         |
                   +-> Cancelled             +-> Paused
```

### Tournament operations

- Allocate initial seats fairly.
- Start tables from a consistent tournament state.
- Advance blind and ante levels centrally.
- Notify tables and clients before level changes and breaks.
- Balance tables when player counts diverge.
- Break short tables and move players at safe hand boundaries.
- Preserve each player's stack across seat moves.
- Prevent a moved player from being dealt into two tables.
- Track eliminations and deterministic finishing order.
- Consolidate to a final table.
- Support hand-for-hand play near configured ranking boundaries if required.
- Recover cleanly when a table server or player connection fails.

Host privileges end at configuration and lifecycle commands accepted by the
tournament controller. All start, pause, cancel, level, seating, gameplay,
elimination, payout, and completion transitions remain server-authoritative,
validated, idempotent where retried, and visible to authorized clients.

Table balancing needs a documented algorithm covering which table supplies a player, which player moves, the destination seat, and how blind position is treated.

## Network protocol

### Transport

Use a persistent bidirectional connection over TLS. WebSocket with JSON messages is appropriate for the first protocol because it is straightforward to inspect, test, and evolve.

### Command envelope

Every gameplay command should include:

- Protocol version
- Player session ID
- Table ID
- Tournament ID when applicable
- Hand ID
- Client-generated command ID
- Expected table revision
- Command payload

Command IDs make retries idempotent. Revisions prevent actions based on stale client state.

### Protocol domains

Commands and events should be separated into:

- Authentication/session messages
- Lobby messages
- Table and seating messages
- Hand and action messages
- Tournament messages
- Connection and recovery messages

### State delivery

- Send ordered events during ordinary play.
- Send complete player-specific snapshots on initial connection and reconnection.
- Use one monotonically increasing revision per table.
- Include explicit command acceptance or rejection.
- Allow lobby subscriptions without joining a table.
- Route a player only the table and tournament information they are authorized to observe.

## Hidden information and player views

The server owns complete internal state. A projection layer must produce a distinct view for each player and spectator.

A player view may contain:

- The player's own hole cards
- Public community cards
- Public seat, stack, bet, pot, timer, and action information
- Revealed showdown cards
- Legal actions only for that player when acting

It must not contain:

- Other players' unrevealed hole cards
- Undealt cards or deck order
- Server random-number state
- Private reconnect credentials belonging to another player
- Mucked cards when the rules say they remain private

The complete internal state type should not be usable directly as a network response.

## Reconnection and timeouts

- Maintain heartbeats and visible connection state.
- Issue a private reconnect token separate from table join codes.
- Reconnect a player to their session, table, seat, and tournament assignment.
- Restore state from a fresh player-specific snapshot.
- Recommended disconnect grace period is 60 seconds.
- Action clocks may pause for a limited grace period or continue according to table policy.
- After timeout, check when legal and otherwise fold.
- Tournament clocks continue independently of individual disconnections.
- Repeated disconnects may receive less grace under an explicit anti-abuse policy.
- Duplicate, delayed, or reordered commands must not apply twice.

## Architecture

```text
Terminal clients
       |
Connection gateway and session routing
       |
Lobby service ---- Tournament controllers
       |                    |
       +---------- Table actors ----------+
                          |
                    Poker rules engine
                          |
                 Persistence and event log
```

Recommended Rust workspace structure:

```text
crates/
  poker-core/        Pure seat-based rules and state transitions
  poker-protocol/    Versioned commands, events, errors, and views
  poker-server/      Connections, sessions, routing, and runtime
  poker-lobby/       Table discovery, creation, and waiting lists
  poker-table/       One serialized authority per active table
  poker-tournament/  Registration, levels, balancing, and standings
  poker-client/      Connection, command, revision, and recovery logic
  poker-tui/         Terminal presentation and input
```

### Required refactors from the current code

- Replace `Player::Human` and `Player::Bot` with stable `PlayerId`, `SeatId`, and controller types.
- Replace fixed `player_*` and `bot_*` fields with seat-indexed collections.
- Generalize button, blind, and turn traversal over eligible seats.
- Implement per-player street and hand contributions.
- Implement main-pot and side-pot construction.
- Make legal-action calculation accept the acting seat.
- Separate deck and full state from player-specific views.
- Move bot decisions behind the same command interface used by human controllers.
- Change the TUI from direct state mutation to command submission and event processing.
- Separate visual animation timing from authoritative action deadlines.
- Update statistics to be player- and game-mode-specific.

Each table should behave like a single-threaded actor: it receives validated commands in order and emits deterministic events. Many table actors may run concurrently, but a single table must never process two state transitions simultaneously.

## Terminal user experience

The TUI requires interfaces for:

- Server connection and display-name selection
- Lobby browsing and filtering
- Table creation
- Masked, case-sensitive password entry for selected protected games
- Nine-seat table layout
- Seat selection and buy-in
- Waiting list status
- Ready, sit-out, return, rebuy, and leave controls
- Player names, stacks, connection status, and action timers
- Main and side-pot display
- Turn and legal-action feedback
- Reconnecting and state restoration
- Tournament lobby, level clock, standings, table assignment, and seat-move notices
- Clear validation and server errors

The nine-seat layout must remain usable at the project's supported minimum terminal size. If it cannot, the client must provide a compact layout or a clear minimum-size message.

## Statistics and hand histories

- Associate statistics with a player identity and game mode.
- Record only server-accepted actions.
- Track position-aware statistics across all seats.
- Keep ring-game and tournament results separate.
- Store complete public hand histories with stable table, hand, seat, and player identifiers.
- Never include unrevealed private cards in public histories.
- Define how disconnect folds, sitting out, and walk hands affect statistics.
- Server-wide rankings require authenticated persistent identities and are not part of the guest-only MVP.

## Security and fairness

- Use cryptographically secure server-side shuffling.
- Never send complete game state to a client.
- Use TLS for public deployment.
- Use unguessable session and reconnect tokens.
- Rate-limit connections, table creation, joins, and gameplay commands.
- Validate names and bound every message and collection size.
- Reject malformed, stale, illegal, and out-of-turn commands without mutation.
- Prevent replayed commands from affecting state twice.
- Log accepted actions and outcomes for audit and debugging.
- Do not expose hole cards or deck order in ordinary production logs.
- Isolate tables so one table cannot read or corrupt another table's state.
- Treat all client clocks, chip totals, action availability, and outcomes as untrusted.

Provably fair commit/reveal shuffling may be added later. Server-authoritative secure randomness is sufficient for the initial play-money platform.

## Persistence and recovery

For a multi-table service, the server should persist enough information to recover player sessions and active tables after a process failure.

Minimum durable records:

- Player/session identity metadata
- Table configuration and lifecycle
- Accepted game commands or resulting domain events
- Periodic table snapshots
- Tournament configuration, registrations, levels, assignments, stacks, and standings

Recovery must:

- Replay only accepted events after the latest snapshot.
- Preserve command idempotency across restarts.
- Restore table revisions.
- Never reshuffle or redeal an in-progress hand.
- Reconnect players to the recovered table state.
- Prevent one recovered player from appearing at multiple tournament tables.

Ephemeral recovery may be acceptable for an early private alpha, but it is not appropriate for a public multi-table tournament service.

## Scalability and operations

- Tables are independent scheduling and failure-isolation units.
- Route all commands for one table to exactly one current authority.
- Allow different tables to process concurrently.
- Partition or shard by table ID when one process is insufficient.
- Maintain a directory mapping table IDs to their current authority.
- Support graceful table migration only after persistence and recovery are proven.
- Lobby updates must be incremental rather than broadcasting the entire table list.
- Tournament controllers must coordinate tables without directly mutating their hand state.
- Define capacity targets for concurrent connections, active tables, and active tournaments.
- Measure connection count, active tables, command latency, action timeouts, reconnects, rejected commands, snapshot age, and recovery failures.
- Graceful shutdown must stop new registrations, drain or snapshot tables, and notify clients.

## Testing requirements

### Rules tests

- Every occupancy from two through nine players
- Every button and blind position
- Heads-up transition when a three-handed table loses a player
- Folded, all-in, sitting-out, empty, and disconnected seat traversal
- Multiway checks, bets, calls, raises, and folds
- Full raises and short all-ins that do or do not reopen action
- Main pot plus multiple side pots
- Different winners for different side pots
- Ties and odd chips in every pot
- Players joining and leaving at hand boundaries
- Blind debt and configured button policy
- Showdown reveal and muck policy

### Invariant and property tests

- Total chips remain conserved.
- Every card in a hand is unique.
- Only an eligible active seat can act.
- Every pot contains the correct contributions and eligible players.
- No player wins a pot for which they are ineligible.
- Rejected commands do not change state or revision.
- One command ID produces at most one transition.
- A player is dealt into at most one tournament table at a time.
- Private views never expose another player's hidden cards.

### Protocol and integration tests

- Serialization and protocol-version compatibility
- Two through nine real clients completing hands
- Many tables completing hands concurrently
- Lobby creation, listing, filtering, joining, and waiting lists
- Reconnect during every street and between hands
- Duplicate, delayed, stale, and reordered commands
- Process restart and table recovery
- Slow-client isolation
- Table actor crash isolation
- Tournament seating, balancing, breaking, and final-table consolidation
- Blind-level changes and scheduled breaks
- Simultaneous table completions during balancing

### Load and soak tests

- Target concurrent connections and active tables
- Nine-player tables with rapid automated actions
- Lobby subscription fan-out
- Mass reconnect after a server restart or network interruption
- Long-running tournaments spanning many blind levels
- Memory stability after repeated table creation and closure

## Acceptance criteria

The multi-table ring-game release is complete when:

- Tables support every occupancy from two through nine.
- Many tables can operate concurrently without cross-table state leakage.
- Players can create, discover, join, sit at, leave, and reconnect to tables.
- Multiway turn order, betting, all-ins, side pots, ties, and odd chips are correct.
- No client receives another player's hidden cards.
- Invalid, stale, out-of-turn, and duplicate actions cannot change state incorrectly.
- A disconnected player can reclaim the correct seat and state.
- Server restart recovery does not redeal or corrupt active hands when durability is enabled.
- The terminal UI presents nine seats and multiple pots clearly.
- Existing offline bot mode continues to work through the generalized engine.

The multi-table tournament release is complete when:

- Players can register and receive initial table assignments.
- Blind and ante levels advance consistently across tables.
- Tables balance and break without duplicating or losing players or chips.
- Player stacks survive seat moves exactly.
- Eliminations and standings are deterministic.
- Play consolidates correctly to a final table and winner.
- Tournament state recovers safely after server failure.

The earlier functional single-table tournament target is complete when:

- The installed Host setup configures and previews bounded entrants/table size,
  starting stacks, starting blinds/antes, level timing/schedule, breaks, and a
  play-money payout/result structure before the configuration locks.
- Two through nine players can create or join, register, and receive one unique
  seat and complete starting stack through the installed product.
- One authoritative schedule advances blinds, antes, and breaks exactly once at
  documented between-hand boundaries.
- Ordinary timeout, disconnect, reconnect, action validation, and private-view
  rules remain effective for every entrant.
- Each player is eliminated at most once; finishing order, remaining count,
  final standings, configured play-money payouts, and one winner are
  deterministic.
- Chips, cards, entrants, awards, and placements reconcile through the final
  hand.
- A controlled between-hand restart preserves configuration, registrations,
  schedule, table, stacks, eliminations, and standings without replay.
- Production terminal evidence covers the complete registration-to-winner
  journey without claiming balancing, movement, breaking, consolidation, or
  hand-for-hand support.

## Recommended delivery plan

### Phase 1: General two-to-nine-seat rules engine

- Introduce player and seat identifiers.
- Generalize position and turn traversal.
- Implement multiway betting and raise-reopening rules.
- Implement side pots and multiway showdown resolution.
- Add deterministic tests and invariants.
- Port the offline bot mode to the new engine.

### Phase 2: One networked table

- Define versioned commands, events, snapshots, views, and errors.
- Implement one authoritative table actor.
- Connect up to nine clients.
- Add reconnection, timeouts, and private-state redaction.

### Phase 3: Multi-table ring-game server

- Add lobby, table creation, discovery, seating, waiting lists, and expiration.
- Run many isolated table actors.
- Add persistence, recovery, metrics, limits, and operational controls.
- Update the TUI for lobby navigation and nine-seat play.

### Phase 4a: Functional single-table tournament

- Add bounded Host setup and preview for capacity/table size, starting stacks,
  starting blinds/antes, level timers/schedules, breaks, and play-money payout
  structure; lock one validated configuration at start.
- Add idempotent registration and cancellation transitions.
- Implement one-table initial seating, eliminations, standings, and winner.
- Integrate installed Host/Join, tournament state, reconnect, and results.
- Add one-controller/one-table recovery and adversarial integration tests.

### Phase 4b: Multi-table tournament expansion

- Add deterministic balancing, atomic movement, route handoff, and table
  breaking.
- Add final-table consolidation and hand-for-hand behavior where required.
- Extend recovery and adversarial tests across coordinated table authorities.

### Phase 5: Public hardening

- Add durable identities if rankings or long-lived balances are required.
- Complete deployment, TLS, monitoring, abuse controls, capacity tests, and incident procedures.
- Publish compatible client and server versions.

## Decisions still requiring confirmation

1. Does “multi-table” mean multiple independent ring-game tables, multi-table tournaments, or both in the first release?
2. Should tables allow cash-style leave-and-rebuy behaviour, tournament freezeouts, or both?
3. Are tables public, private, or both?
4. May one user play at several tables simultaneously?
5. Are guest names sufficient, or are durable accounts required?
6. What buy-in, blind, ante, missed-blind, button, and showdown policies should be authoritative?
7. Must active games survive server restarts in the first release?
8. What are the initial capacity targets for players, tables, and tournaments?
9. Which setup presets and min/max bounds ship for starting stacks, starting
   blinds/antes, level durations, breaks, and payout places; how are payout odd
   units rounded?
9. Are bots permitted at public tables, private tables only, or offline only?
10. Are spectators and delayed hand histories required?

The first decision is the largest remaining product ambiguity. Tournament orchestration adds a substantial layer beyond hosting many independent nine-handed tables, even though both use the same underlying table engine.


## Current showdown policy (2026-09-08, PokerStars alignment)

ADR 0017 now uses the called river aggressor first, otherwise first active seat
left of the button. Beaten hands automatically muck for humans and AI; winners
and ties table. H during betting selects always-show for this hand. No five-second
choice window or mandatory heads-up input remains. Actual reveals/runout steps
pause 1.5 seconds; muck skips add no pause. All-in mandatory exposure, no rabbit
hunting, best-five green brackets and private public histories remain in force.
Protocol/wire v4 peers are required. This supersedes the earlier best-first and
optional five-second procedure without changing chips, betting or history access.

### Sprint 19 lobby acceptance boundary (2026-09-09)

Join Game opens a directory on one remembered dedicated server. This does not
require broadcast discovery across VLANs. Sprint 20 supersedes the original loopback-only restriction: the updated client
automatically uses verified TLS to the supplied Fedora host at 192.168.5.250:6969.
Administrative SSH is independent of player connections. Both open and password-protected games
are listed with game name, field occupancy, blinds, availability, and access type.
Refresh preserves table identity; full/started games reject new registration on
the server. Ring rows explain current client support instead of silently hiding.
Host chooses name and optional 4-96 printable ASCII password (spaces retained).
Password is masked, omitted from public summaries and client persistence, and
stored only as a salted server verifier. Esc/disconnect releases pre-start
registration; a concurrent start locks registration and uses normal game rules.
