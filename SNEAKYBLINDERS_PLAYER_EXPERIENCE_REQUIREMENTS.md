# Current direction - 2026-09-09

Dedicated-server Host/Join supersedes the embedded This-computer host lifecycle:
one server process owns multiple independent games, and all players are clients.
Sprint 18 proves this locally; protected LAN and Pi service deployment follow.
Quick Practice currently uses nine seats and passive bots; the early heads-up /
balanced-bot promise below is superseded. Custom Practice follows DS integration.
The unified appearance supersedes selectable theme/motion requirements.
Durable Practice saves and active-hand crash recovery require explicit refinement.

# Sneaky Blinders Player Experience Requirements

Status: Accepted product direction; unresolved reach, HUD, and content decisions remain gates

Date: 2026-09-02

Scope owner: Product owner

Implementation status: 74/121 accepted through Sprint 16; full experience incomplete

Technical UI map:
[Ratatui and TachyonFX UI Map](docs/development/RATATUI_TACHYONFX_UI_MAP.md)

## Product intent

`sneakyblinders` is the product. A player installs it once, opens any terminal,
types one command, and completes every ordinary journey inside one coherent
Ratatui application:

```text
sneakyblinders
```

Players must not need to know that separate server, client, review, transport,
credential, or persistence components exist. Those components may remain as
internal development tools, but they are not installed or documented as player
entry points.

### Current executable boundary

The repository now owns a `sneakyblinders` executable. With no arguments it
opens the production Home screen and supports repeatable Quick Practice plus a
private two-to-nine-human Host/Join single-table freezeout. Every player submits
ordinary commands through a distinct authorized projection; the production
table renderer receives only the local player's view.

Home, Settings, Help, private Host/Join, the persistent player-facing table
console, automatic hand rollover, profile persistence, capability fallbacks,
and terminal restoration are accepted across CMD, PowerShell, and Git Bash.
Study remains honestly unavailable. Controls are functional and replayable but
not yet intuitive; Custom Practice, Study, Range Explorer, packaging, public
reach, and final usability remain open. Responsive table convergence is
accepted through Sprint 16.

The product remains play-money No-Limit Texas Hold'em. Real-money wagering is
out of scope.

## Experience principles

1. One command, one visual language, and no shell choreography.
2. The first useful screen appears without mandatory flags or configuration.
3. Every screen explains what the player can do next.
4. Advanced poker and network concepts are progressively disclosed.
5. Practice, hosted, and joined games use the same authoritative rules and
   table presentation.
6. Hosting never grants access to another player's hidden information.
7. Secrets and internal identifiers are handled by the application, not copied
   through command-line arguments.
8. Failures return the player to a recoverable screen with a useful explanation.
9. Functional keyboard controls are not enough: primary actions are
   self-describing, discoverable without external documentation, and validated
   through qualitative first-use observation.

## Player-facing command contract

### Required behavior

- A user-level installation places exactly one player-facing executable named
  `sneakyblinders` on `PATH`.
- Running it with no arguments opens the Home screen.
- It works from any current directory and does not depend on the repository,
  Cargo, source files, or a pre-opened server terminal.
- Normal Practice, Host, Join, Study, Settings, Help, reconnect, and quit
  journeys require no command-line flags.
- The executable owns terminal setup and always restores the terminal on clean
  exit, error, panic, or interrupt.
- `--help` and `--version` may remain for shell conventions, diagnostics, and
  support. They must not become alternate player workflows.
- `poker`, `terminal-poker`, and `poker-client` are retired from player-facing
  installation and documentation. A server executable may remain an internal
  deployment artifact.

### Installation and upgrade

- Installation is user-level by default and does not require an administrator
  shell when the platform permits it.
- Upgrade is atomic: a failed upgrade leaves the previous executable usable.
- Upgrades preserve settings, authorized hand histories, statistics, saved
  credentials, and recoverable hosted-table state.
- The app shows its version and data location in Help or Settings.
- Uninstall removes the executable but asks separately before deleting player
  data.
- No update is downloaded or installed without explicit user consent.

## Information architecture

```text
Home
|-- Continue                      shown only when a journey can be resumed
|-- Practice
|   |-- Quick Practice
|   |-- Custom Practice
|   `-- Resume Practice
|-- Host Game
|   |-- Create Ring Table
|   |-- Create Tournament
|   |-- Host Lobby
|   `-- Hosted Table
|-- Join Game
|   |-- Enter Invite
|   |-- Recent Games
|   |-- Browse Public Tables      only when a configured service supports it
|   `-- Joined Table
|-- Study
|   |-- Recent Hands
|   |-- Hand Replay
|   |-- Statistics
|   `-- Learn
|-- Settings
|-- Help
`-- Quit
```

The four primary calls to action are **Practice**, **Host Game**, **Join Game**,
and **Study**. Continue is contextual, not a fifth mode.

## Global shell requirements

### Home

- Show the product name, version, current profile/display name, and connection
  state without exposing credentials or internal session identifiers.
- Show the four primary modes without scrolling at the supported minimum size.
- Explain the selected mode in one short sentence.
- Show Continue only for a locally resumable practice session, reconnectable
  joined table, or recoverable hosted table.
- Show release-stage limitations honestly. An unavailable online service is
  disabled with a reason; it is not presented as a working action.

### Navigation

- Arrow keys move, `Enter` selects, and `Esc` returns to the previous safe
  screen.
- `Tab` and `Shift+Tab` move between fields where forms are present.
- `?` opens contextual help.
- `Q` requests quit from non-text contexts; active games and hosted authorities
  require confirmation or a safe leave/drain choice.
- Key hints remain visible and change with context.
- Text fields support editing, clipboard paste, validation, and masked secret
  entry where appropriate.
- Mouse support is optional; every journey must be keyboard-complete.

### Visual system

- Menus, forms, lobbies, tables, replay, dialogs, notifications, and errors use
  one Ratatui theme and shared components.
- Focus, selection, disabled, warning, success, and destructive states are
  visually distinct and do not rely on color alone.
- No action is represented only by an unexplained letter.
- At-table play uses one responsive component tree from the accepted minimum
  through large terminals; resizing must never select a different table UI.
- The supported at-table envelope is a width/height staircase: 80x30, 72x32,
  64x36, or 56x40 and any larger viewport satisfying the corresponding band.
  Below it, show a clear minimum-size message rather than a partial or corrupted
  table.
- The table is portrait-first. Wider terminals enrich spacing, labels, and
  console depth without stretching the felt into a landscape design, adding a
  wide-only side rail, or moving primary landmarks.
- Network waiting, reconnecting, host draining, turn deadline, all-in, pots,
  hand outcomes, and saved-state status are explicit.

## Mode requirements

### Practice

Purpose: play immediately against computer-controlled opponents without network
setup or another human.

#### Quick Practice

- Starts a heads-up 100-big-blind game against a balanced bot in at most two
  selections from Home.
- Uses the same validated command boundary and authoritative multiway rules as
  hosted games; the legacy heads-up adapter is not the long-term authority.
- Keeps the player at the table, prints the reconciled outcome to the table
  console, and starts the next eligible hand automatically after a short
  readable showdown hold.

#### Custom Practice

- Configure two through nine total seats, one human seat, one through eight bot
  seats, starting stack, blinds, and bot difficulty/profile.
- Validate values inline and offer sensible named presets.
- Permit private-table bot behavior only; practice bots never imply that public
  network tables contain bots.
- Support a deterministic training seed only in an explicitly labelled Study or
  Practice advanced option. Ordinary play uses production randomness.

#### Practice lifecycle

- Pause between actions, save locally, resume from Home, restart, or end the
  session.
- Record player-authorized histories and mode-specific statistics.
- Clearly attribute Practice hand/session summaries in the table console and
  keep their stored statistics separate from human ring-game results unless the
  player chooses an aggregate Study view.
- A bot receives only its authorized seat projection and submits ordinary legal
  commands.

### Host Game

Purpose: create and supervise a private or discoverable server-authoritative
ring game without manually starting a server process.

#### Create Ring Table

- Collect table name, visibility (public, unlisted, or private), seat count from
  two through nine, minimum players, starting stack, blind preset, and optional
  private-table bots.
- Explain each visibility choice before creation.
- Private tables generate their access secret inside the application. The
  secret is never passed through process arguments, environment variables,
  logs, screenshots, health output, or checkpoints.
- Advanced values have policy-defined bounds and inline validation.
- Hosting scope is explicit before confirmation:
  - **This computer** is the first supported integrated slice.
  - **Local network** is enabled only after safe bind, firewall, discovery, and
    credential transport requirements are met.
  - **Online** is enabled only after TLS, service identity, deployment,
    operations, and abuse controls pass their release gates.

#### Create Tournament

- Reuse the Host reach, invite, supervision, and authority boundaries rather
  than exposing a separate server command.
- Before configuration lock, let the host edit bounded entrant/table capacity,
  starting stack, starting blinds/antes, blind-level durations and schedule,
  breaks, and the play-money payout/result structure.
- Show a complete setup preview, inline validation, and named presets. Invalid
  or non-reconciling combinations cannot open registration.
- Once started, the configuration is server-owned. Host controls cannot alter
  cards, pots, current bets, live stacks, action rights, eliminations, awards,
  or the authoritative clock outside accepted lifecycle commands.

#### Host supervision

- The app starts, monitors, and stops the authority without another visible
  terminal or player command.
- The host joins through the same player protocol and projection as every other
  player; host controls cannot inspect or alter cards, deck order, pots, stacks,
  actions, or outcomes.
- The Host Lobby shows the table configuration, occupied/open seats, waiting
  list, readiness, connection state, and a safe share action.
- Other players receive one human-readable invite value. They do not need to
  assemble an address, table ID, join code, session label, seat, or credential
  path.
- Closing a host with active players presents explicit choices and performs a
  bounded graceful drain. It never silently abandons or corrupts a hand.
- A recoverable between-hand checkpoint is maintained using the accepted
  private-beta boundary. Active-hand recovery remains separately gated.
- Authority failures show diagnosis and recovery choices in the app and do not
  leave an unowned background process.

### Join Game

Purpose: enter a game from a single invitation or a safe public directory.

#### Entry paths

- **Enter Invite** accepts one pasteable invite value and validates it locally
  before attempting a connection.
- **Recent Games** lists safe aliases such as host/table name and last played
  time. It never displays bearer credentials.
- **Browse Public Tables** lists only fields approved for public projection and
  is hidden or disabled when no directory service is configured.
- An Advanced screen may accept a host address and table identifier for
  development or self-hosted use, but this is not the primary journey.

#### Admission and play

- Ask for a display name on first use and remember it locally. A display name is
  presentation, never authorization.
- Show table name, blinds, stack, occupancy, visibility, and queue state before
  joining.
- Allow automatic seating by default and optional seat selection when the table
  policy permits it.
- Explain full, unavailable, wrong-code, version-mismatch, timeout, and unsafe-
  host failures in player language with retry/back choices.
- Store server-issued credentials in the platform data directory with
  user-only access where supported.
- Reconnect automatically with the current scoped credential, handle rotation,
  and return to the joined table without asking for a session label or token.
- Waiting-list promotion, between-hand admission, sit-out, leave, and return are
  visible and preserve the accepted server policies.

### Study

Purpose: understand previous decisions and improve without entering a live game.

#### Recent Hands

- List completed hands from Practice and authorized hosted/joined play.
- Filter by mode, date/session, table size, result, pot size, and marked hands.
- Show an honest empty state and explain how histories are created.
- Importing third-party formats is a later capability, not an MVP dependency.

#### Hand Replay

- Replay one completed hand from initial state through every public action,
  street, pot construction, showdown, and award.
- Provide previous/next action, first/last action, street jump, and autoplay.
- Reconcile board, pot, contributions, stacks, actor, legal context, and result at
  each step.
- Show the local player's legitimately known hole cards and only legitimately
  revealed opponent cards. Mucked cards, deck order, random state, credentials,
  and private server state are never recoverable from Study data.
- Permit notes and a marked-for-review flag without changing the immutable hand
  record.

#### Statistics

- Separate Practice, human ring-game, and tournament statistics.
- Show sample size and avoid presenting tiny samples as reliable strategy
  conclusions.
- Include the currently tracked core measures: hands, profit in big blinds,
  BB/100, VPIP, PFR, 3-bet, c-bet, fold-to-c-bet, aggression, showdown rate,
  showdown win rate, and largest won/lost pots.
- Permit session, date, table-size, and mode filters.

#### Learn

- Provide an offline rules and controls reference covering actions, blinds,
  positions, hand rankings, side pots, all-ins, showdown, and table lifecycle.
- Link contextual help from the table and replay views into the relevant topic.
- Solver recommendations, opponent profiling, neural coaching, quizzes, and
  externally sourced strategy content are later product decisions.

#### Range Explorer

- Present a keyboard-navigable 13 x 13 starting-hand matrix with pairs on the
  diagonal and suited/offsuit combinations on opposite sides.
- Select position, table size, effective stack, blind/ante structure, and prior
  action scenario before interpreting a range.
- Distinguish fold, raise, shove/all-in, call, and mixed-frequency actions using
  both semantic color and text/glyph/border treatment.
- Show action-frequency summaries, exact focused-cell frequencies, combination
  count, strategy source, version, and provenance.
- Use one legible primary matrix at 120 x 40; wide layouts may add positional
  comparison summaries.
- Treat the supplied range image as a visualization reference only. Strategy
  content requires a separately approved, versioned data pack and must never be
  invented or described as optimal without evidence.

### In-game table shell

- Practice, hosted, and joined play share one table component driven by a
  player-authorized projection.
- Use the complete terminal area and one stable composition order: status/header,
  portrait table stage, player console, legal actions, contextual footer.
- Preserve stable clockwise two-to-nine-seat identity and card language through
  resize; hero remains bottom-centre.
- Adapt constraints, truncation, and visible console depth continuously. Do not
  switch between compact, standard, or wide table renderers.
- At 56x40, 64x36, 72x32, 80x30, and 120x40, the same landmarks, controls,
  information hierarchy, and visual table proportions remain recognizable.
- Provide fold, check/call, bet/raise with amount entry, and all-in controls only
  when legal and authoritative.
- Show whose turn it is, the player's deadline, pending command state, accepted
  result, connection state, reconnect state, and why controls are disabled.
- Show main and side pots, per-seat stack/contribution/state, button/blinds,
  board, street, table name, hand identity, and a persistent scrollable table
  console.
- The table console is a player feature, not a diagnostic log. It prints
  dealer notices, blinds, accepted actions, street/board transitions, pot
  changes, awards, stack/session reconciliation, connection notices, and safe
  rejections using only the viewer's authorized projection and public events.
- Never print wire direction, stream state, command IDs, revisions, seeds,
  credentials, internal actor state, or hidden cards in the table console.
- Keep at least 200 bounded session messages and show the latest messages by
  default; `PageUp`/`PageDown` and `Home`/`End` browse history without changing
  authoritative game state.
- Reserve separately labelled console channels for future chat and hand-history
  navigation. Chat is not an alias for dealer/system output and requires its own
  identity, bounds, moderation, abuse, privacy, and retention decisions before
  activation.
- Provide an in-game menu for Help, Settings, Sit Out/Return, Leave Table, and
  Quit. Host-only operational actions remain separated from poker actions.
- At terminal outcome, keep the table visible, print awards and stack/session
  reconciliation to the console, then start the next eligible hand in-place.
  Review Hand is a secondary Study action; Home/Leave remains an explicit
  player choice rather than an interstitial Results screen.
- Hold the table for three one-second presentation stages before rollover:
  legitimate showdown reveals with folded/mucked demarcation, winning-hand
  determination with every chop winner and each winner's playing five cards
  highlighted, then exact per-seat chip awards. Presentation timing never
  delays or changes authoritative settlement.
- Use stable nine-seat perimeter anchors with the hero bottom-center, community
  cards and pots central, and legal actions directly below the hero.
- Support an optional HUD layer without moving seat geometry. Live human-table
  HUD is off by default until its fairness policy is approved; any enabled
  opponent statistic must derive only from locally observed public actions and
  include its sample size.

### Settings and Help

- Settings include display name, theme/color mode, reduced animation, sound if
  later introduced, key reference, confirmation preferences, and safe data
  management.
- Network secrets are never shown in full in Settings.
- Resetting statistics/history is separate from forgetting credentials and
  requires confirmation with an exact scope summary.
- Help includes controls, mode explanations, version, data paths, privacy model,
  diagnostics export, and known release-stage limitations.
- Diagnostics export uses an explicit safe allowlist and excludes cards that
  were never revealed, credentials, join secrets, command payloads, and random
  state.

## Application state and lifecycle

- The shell has one explicit navigation state and one optional active activity:
  practice authority, hosted authority, joined network session, or study replay.
- Mode transitions perform defined cleanup; navigating Home must not orphan a
  server, socket, terminal mode, checkpoint writer, or credential rotation.
- Continue resumes only after validating the saved state and rules/schema
  version. Invalid state fails closed and offers retain-for-diagnostics or safe
  removal.
- `Ctrl+C`, terminal close where detectable, and ordinary Quit share the same
  bounded shutdown path.
- A host authority and its player client have independent roles even when they
  share one operating-system process.
- Multiple invocations by the same OS user must not corrupt shared settings,
  histories, credentials, or checkpoints. File publication is atomic and locks
  or per-instance ownership are explicit.

## Local data requirements

Use the platform's user configuration and data directories, never the current
working directory by default. Logical stores are separated:

- settings and profile presentation;
- scoped credentials and recent-host aliases;
- player-authorized hand histories and notes;
- mode-specific statistics;
- resumable practice state;
- hosted-table checkpoint and safe server history;
- privacy-safe logs and diagnostics.

Every persisted format is versioned, bounded, and either migratable or rejected
with a recovery explanation. Credentials and access secrets are not embedded in
general settings, histories, statistics, logs, or host checkpoints.

## Network and release boundaries

The menu does not make the current loopback transport internet-safe. Capability
labels are governed by executable release evidence:

- **This computer:** may use the current bounded server/client transport once it
  is integrated and supervised by the shell.
- **Local network:** requires an explicit safe-bind design, firewall guidance,
  reachable-address selection, transport threat review, and process tests across
  two machines or isolated network namespaces.
- **Online:** requires TLS, server identity, secure invitation routing, hosted
  infrastructure, rate limiting, abuse controls, observability, capacity,
  support ownership, and explicit go-live approval.

Unavailable capabilities must be disabled or omitted. The product must never
encourage a player to expose the current loopback/private-beta authority directly
to the internet.

## Security, privacy, and authority

- The server remains the sole authority for shuffle, cards, legal actions,
  timing, pots, awards, and stacks in hosted/joined play.
- Local Practice uses the same authority boundary even if all components execute
  in one process.
- Player views and Study records are constructed from explicit authorized/public
  projections, never serialized internal state.
- Join codes and credentials are entered through masked or deliberately revealed
  UI fields and cleared from transient buffers when no longer needed.
- The host cannot promote itself to spectator/admin access that reveals hidden
  cards.
- Logs and support bundles use explicit safe schemas, size bounds, retention,
  and secret-negative tests.
- Telemetry is off by default unless a later policy obtains explicit player
  consent and defines collection, retention, and deletion.

## Reliability and performance

- Home should render within two seconds on a supported local machine after a
  warm start; any slower initialization runs asynchronously with visible status.
- Key input feedback should appear in the next rendered frame; network acceptance
  remains authoritative and may arrive later.
- Long host discovery, connection, history loading, and recovery operations do
  not freeze rendering or input.
- Repeated navigation and ten consecutive start/play/end cycles do not leak
  terminal state, workers, sockets, file handles, or background authorities.
- Existing two-hour 8-table/32-session capacity and privacy invariants remain
  regression gates for backend-affecting shell work.

## Accessibility and usability

- Critical state is conveyed through text/symbols as well as color.
- Provide a color-blind-safe theme and reduced-animation mode.
- Avoid time-limited menu choices. Poker action timers are server policy and show
  an accessible countdown/status warning.
- Error messages contain: what happened, whether state is safe, and the next
  available action.
- A first-time player can start Quick Practice without reading external docs.
- A host and guest can complete a private-table journey using only on-screen
  instructions and one invite value.
- A first-time player can discover check/call, variable raise sizing, fold,
  leave/return, console history, Host, and Join from visible context without
  memorizing unexplained command letters.

## End-to-end acceptance journeys

The integrated experience is not accepted until all applicable journeys pass in
the installed executable, not only fixtures or component tests.

1. **Clean first run:** from an arbitrary directory, `sneakyblinders` opens Home,
   creates user data safely, explains navigation, and exits with the terminal
   restored.
2. **Quick practice:** a new player starts and completes one bot hand, reads the
   outcome in the table console, and sees the next hand begin automatically
   without an interstitial screen or process restart.
3. **Custom nine-seat practice:** one player and eight authorized bots complete a
   hand with correct private views and chip conservation.
4. **Host private table:** the app creates and supervises a private table, shows
   one shareable invite, joins the host as an ordinary player, and drains safely.
5. **Join private table:** a second installed process pastes the invite, enters a
   display name, waits or seats, completes a hand, and sees the same reconciled
   public outcome.
6. **Reconnect:** a joined client loses transport, automatically presents clear
   status, uses a rotated scoped credential, and returns without duplicate
   actions or token entry.
7. **Study:** the player opens that completed hand, replays the exact action
   trajectory, sees their own known cards, and cannot recover another player's
   mucked cards or any credential.
8. **Persistence:** settings, histories, statistics, credentials, and resumable
   state survive an app restart and compatible upgrade.
9. **Failure recovery:** wrong invite, unavailable host, full table, incompatible
   version, corrupt local history, and rejected checkpoint each produce a safe,
   actionable screen.
10. **Installed-artifact gate:** no player-facing journey invokes or documents
    `poker-server`, `poker-client`, `poker`, `terminal-poker`, Cargo, repository
    paths, environment secrets, or manually selected session identifiers.

Every future sprint review for this experience must capture the installed
`sneakyblinders` path, visually inspect the real Ratatui screens, and retain the
project's continuous single-hand PDF trajectory standard.

## Refined delivery slices and estimate

The shell/table/install spike and first human terminal feedback replace the
original 89-point placeholder with the 108-point E12 refinement. The
post-Sprint-15 responsive-table refinement adds E12.8b/E12.11c at 13 points,
moving the product reforecast from 660 to 673; no tangent points are
retroactively accepted.

| Slice | Outcome | Points |
|---|---|---:|
| UX1 | Installed shell, route/event model, Home, and terminal-safe lifecycle | 8 |
| UX2 | Versioned local profile/settings/data layout and migrations | 5 |
| UX3a | Projection-safe single-hand Quick Practice | 3 |
| UX3b | Repeatable Quick Practice, table-console outcomes, and automatic next-hand lifecycle | 5 |
| UX3c | Custom Practice with 2-9 seats and bounded bot profiles | 8 |
| UX4 | Host creation, internal authority supervision, host lobby, invite, and safe drain | 13 |
| UX5a | Private-invite/recent Join, admission, waiting, credential storage, and reconnect UX | 8 |
| UX5b | Public discovery and online Join reach | 5 |
| UX6 | Study hub with authorized histories, replay, filters, statistics, notes, and Learn | 13 |
| UX7 | Range Explorer schema, provenance, validation, and synthetic-fixture renderer | 8 |
| UX8 | Shared table shell, player-facing console, contextual help, and clean mode transitions | 8 |
| UX9 | Compatible UI platform, semantic themes, capability fallbacks, presentation-safe motion, and reduced motion | 8 |
| UX10 | Single-artifact packaging, atomic update/uninstall, data preservation, and diagnostics | 8 |
| UX11 | Installed cross-shell journeys, accessibility, usability, and release evidence | 8 |
|  | **Total** | **108** |

UX6 excludes solver and model advice. UX7 does not include researching,
licensing, generating, or validating a real strategic range pack. UX8 may render
an optional HUD state, but live human-table HUD behavior cannot be accepted until
its fairness policy is decided.

The first coherent milestone should combine UX1, UX2, a heads-up subset of UX3,
and the installed-artifact portion of UX8. It should end with the command already
installed on the user's machine opening Home and starting Practice through menus.
Host/Join should follow as one vertical slice rather than separate mock screens.

## Dependencies and conflicts with the current roadmap

- The refined 108-point estimate is incorporated by the 2026-09-01 player-
  experience release rebase, producing a 660-point product backlog.
- Sprint 14 accepted the installed shell and Quick Practice. Product direction
  D-033 makes the complete functional single-table tournament the recommended
  Sprint 15 outcome: UX4 plus private UX5a join all 55 D1 points. UX3c Custom
  Practice and public UX5b reach move behind D1. This sequencing preserves the
  108-point experience estimate and the 55/34 tournament split.
- Multiway Practice now has a first one-human/eight-bot, single-hand vertical
  slice through the accepted multiway authority. Configuration, multiple hands,
  bot profiles, saving, statistics, console channels, and Study/review navigation remain.
- Host/Join can reuse the registry, private tables, credentials, reconnect,
  checkpoint, history, health, and drain work completed through Sprint 13.
- The parallel policy-learning harness is not part of the 108-point player-
  experience estimate. Current Quick Practice opponents are passive,
  projection-bound authority adapters, not a learned model. Any trained
  checkpoint may enter Practice or private Host only after separate quality,
  privacy, promotion, and deployment decisions; public-table bot policy remains
  governed by D-018.
- Online Host/Join depends on unresolved deployment, TLS, identity, abuse,
  capacity, and operations decisions. Menu implementation alone cannot close
  those gates.

## Product decisions required before full commitment

Recommended defaults are shown first; changing them may materially change scope.

1. **Initial Host reach:** This computer first; Local network and Online remain
   disabled until their security/release gates pass.
2. **Practice breadth:** retain the accepted fixed nine-handed Quick Practice,
   then require Custom Practice for two through nine total seats before the
   experience epic closes; Custom Practice is not a D1 gate.
3. **Private bots:** permit them in Practice and explicitly private hosted tables;
   never add them to public tables without a later policy decision.
4. **Invite design:** one opaque, pasteable invite value is the ordinary path;
   manual address/table entry lives under Advanced.
5. **Study MVP:** authorized replay, statistics, notes, and offline rules reference;
   defer solver advice and model-driven coaching.
6. **Host lifetime:** the locally hosted authority runs only while its owning app
   remains active and drains on exit; always-on hosted service is a separate
   deployment mode.
7. **Public product name:** `sneakyblinders` is the accepted working command;
   public distribution still requires ordinary naming and brand-clearance review.
8. **Live HUD policy:** keep it off by default; decide whether public-observation-
   only opponent statistics are acceptable in human games or restricted to
   Practice and Study.
9. **Range strategy data:** build the visualization against synthetic fixtures;
   ship strategic ranges only from an approved, versioned, attributable content
   pack.

## Explicit exclusions from the first integrated experience milestone

- Real money, deposits, withdrawals, prizes, or transferable value
- Public internet exposure or hosted-service deployment
- Tournament registration, balancing, movement, standings, and payouts
- Multiple simultaneous tables in one terminal process
- Spectator mode, chat, moderation, friends, accounts, and password recovery
- Solver integration, external training content, and neural strategy coaching
- Web, graphical desktop, and mobile clients

These exclusions constrain the first milestone, not the long-term architecture.
