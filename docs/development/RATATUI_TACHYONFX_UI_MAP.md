# Ratatui and TachyonFX UI Map

Status: Active technical direction; production table and initial installed shell implemented

Date: 2026-09-01

Companion product requirements:
[Sneaky Blinders Player Experience Requirements](../../SNEAKYBLINDERS_PLAYER_EXPERIENCE_REQUIREMENTS.md)

## Recommendation

Begin the UI work now, before tournament breadth. Use Ratatui as the complete
immediate-mode application and component renderer. Use TachyonFX only as a
post-render presentation layer driven by explicit UI events.

Take one deliberate dependency step before building the new shell:

- upgrade Ratatui from 0.29.0 to 0.30.2;
- upgrade Crossterm from 0.28.1 to 0.29;
- add TachyonFX 0.25.1;
- retain the `ratatui` application crate rather than importing split core crates
  throughout the app;
- prove the upgrade in an isolated visual/compatibility spike before changing
  the product layout.

The project Rust 1.98.0 toolchain exceeds Ratatui 0.30.2's Rust 1.88 minimum.
Ratatui 0.30.2 and TachyonFX 0.25.1 share `ratatui-core` 0.1.2, which avoids
duplicated incompatible buffer/style types. The low-change fallback is to keep
Ratatui 0.29.0 and pin TachyonFX 0.19.0, which directly depends on Ratatui 0.29.0,
but that creates a dependency migration immediately after the shell work begins.

No dependency upgrade is implemented by this map.

## Implemented vertical slice

The approved Variant 01 Portrait Oval is now the sole production network table
renderer at every supported viewport:

- `src/ui/ash_table.rs` owns the card-first table presentation and stable 2-9
  seat perimeter geometry;
- `render_network_view` always selects that renderer; viewport size changes
  constraints and abbreviation only, never the component tree;
- the local authorized seat rotates to bottom-centre while physical seat IDs,
  button/blind markers, contributions, pots, and turn ownership remain intact;
- hole cards are drawn only when present in the authorized projection;
- action labels and enabled styling derive from `MultiwayLegalActions` plus the
  projection client's controls gate;
- pending commands, stream resynchronization, and disconnects visibly disable
  play without exposing command IDs, revisions, random seeds, or transport logs;
- `review-network-client` now captures the same `render_network_view` path used
  by the playable client.

Sprint 16 removed the breakpoint swap and accepted this responsive hierarchy as
the production contract. Below the supported width/height staircase, the same
renderer presents a safe minimum-size explanation rather than a partial table.

The table slice did not add TachyonFX or upgrade Ratatui/Crossterm. A subsequent
installed-shell tangent added production Home and a projection-backed Quick
Practice route, while the shared route/event model, dependency migration,
semantic theme, effects registry, and reduced-motion system remain later work.

## Primary-source basis

- [Ratatui 0.30.2 API documentation](https://docs.rs/ratatui/0.30.2/ratatui/)
- [Ratatui 0.30 modular architecture](https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md)
- [Ratatui 0.30.2 release](https://github.com/ratatui/ratatui/releases/tag/ratatui-v0.30.2)
- [Ratatui application concepts](https://ratatui.rs/concepts/)
- [TachyonFX repository and examples](https://github.com/ratatui/tachyonfx)
- [TachyonFX 0.25.1 API documentation](https://docs.rs/tachyonfx/0.25.1/tachyonfx/)
- [TachyonFX 0.25.1 manifest](https://docs.rs/crate/tachyonfx/0.25.1/source/Cargo.toml)
- [TachyonFX effect manager](https://docs.rs/tachyonfx/0.25.1/src/tachyonfx/effect_manager.rs.html)
- [TachyonFX 0.19.0 manifest](https://docs.rs/crate/tachyonfx/0.19.0/source/Cargo.toml)

## Current UI map

### Dependency snapshot

| Concern | Current implementation |
|---|---|
| TUI renderer | Ratatui 0.29.0 |
| Terminal backend/input | Crossterm 0.28.1 |
| Motion | Hand-written offline event timing; no shared effects system |
| Primary renderer | `src/ui/render.rs`, approximately 1,650 lines |
| Offline app state | `src/ui/app.rs::App` |
| Network app state | `src/ui/network_app.rs::NetworkApp` |
| Offline input | `src/ui/input.rs` plus phase handling in `src/main.rs` |
| Network input | Direct key matching in `src/bin/poker_client.rs` |
| Lobby UI | Public directory view/capture, not an interactive player screen |
| Theme | File-local color constants in `render.rs` |
| Current reviewed at-table envelope | One renderer at 80x30, 72x32, 64x36, 56x40, and larger supported viewports |

### Runtime paths

```text
INSTALLED PLAYER SLICE

sneakyblinders
  -> production Home / honest route availability
  -> Quick Practice
  -> LocalPractice owns one multiway authority and nine authorized sessions
  -> ProjectionClient exposes only the local player's view
  -> render_network_view -> ash_table::render at every supported size
  -> Q/Esc returns Home and restores the terminal on exit

OFFLINE HEADS-UP

src/main.rs
  -> 50 ms loop
  -> App owns legacy game, bot sequencing, overlays, event queue
  -> ui::input::handle_key
  -> ui::render::render(App)
  -> offline table / help / stats / result overlays

NETWORK RING GAME

poker-client
  -> 20 ms loop
  -> NetworkSession polls authoritative wire messages
  -> NetworkApp applies messages to ProjectionClient
  -> NetworkApp::view builds MultiwayReviewView
  -> render_network_view
  -> ash_table::render at every supported size
  -> card-first perimeter / board / projected actions / connection recovery
  -> safe minimum-size state below the supported staircase

LOBBY CAPTURE

poker-client --lobby-list --capture-dir
  -> LobbySession list request
  -> LobbyView
  -> render_lobby_view
  -> one rendered frame, then process exit
```

### Existing presentation surfaces

| Surface | Useful assets to retain | Current limitation |
|---|---|---|
| Offline heads-up table | Strong card faces, felt, actions, raise input, help/stats overlays | Uses the legacy authority and a separate layout/state/input stack |
| Network ring table | Real player projection, 2-9 seats, board, pots, lifecycle, reconnect status | Looks like a diagnostic review screen; production and review view models are conflated |
| Public lobby | Safe allowlisted rows and clear table facts | Capture-only; selection is not interactive |
| Help/stats/results | Existing copy and tracked metrics | Tied directly to offline `App` |
| Review renderers | Deterministic backend and screenshot evidence | Review naming and metadata leak into production presentation structure |

### Structural gaps

1. The initial `sneakyblinders` shell now owns Home and Quick Practice, but the
   full route/activity model and Host, Join, Study, Settings, and Help journeys
   remain incomplete.
2. Offline and network play have separate state, input, lifecycle, and rendering
   loops.
3. `render.rs` owns theme, geometry, widgets, screens, overlays, and test-facing
   rendering in one module.
4. `MultiwayReviewView` serves both production network rendering and evidence
   capture, so technical build/protocol text dominates the live table.
5. The seven-to-nine-seat layout is a 5/4 two-row grid rather than a visual table
   perimeter.
6. Lobby rendering has no focus, filtering, form, modal, or navigation state.
7. There is no reusable component contract, focus model, toast/dialog layer,
   responsive screen policy, or theme abstraction.
8. The always-redraw 20/50 ms loops have no dirty-state scheduler or animation
   clock.
9. Motion is not reduced-motion aware and cannot be deterministically sampled as
   a shared presentation concern.

## Target application architecture

### State and event flow

```text
terminal/network/host/practice events
                |
                v
          AppEvent boundary
                |
                v
       SneakyApp state reducer
       route + focus + modal + activity
                |
                +------> domain/network command intent
                |             |
                |             v
                |      authoritative boundary
                |             |
                |<---- authorized projection/update
                |
                +------> PresentationEvent diff
                               |
                               v
                        UiEffects registry
                               |
                               v
screen/components render stable authoritative state to Ratatui Buffer
                               |
                               v
TachyonFX transforms selected cells/regions using elapsed presentation time
                               |
                               v
                       terminal diff output
```

The reducer decides navigation and player intent. The server/practice authority
decides poker. TachyonFX decides neither.

### Proposed module boundaries

```text
src/ui/
|-- shell.rs                 SneakyApp, route stack, active activity, quit/drain
|-- event.rs                 AppEvent and PresentationEvent
|-- component.rs             screen/component contracts and focus routing
|-- theme.rs                 semantic design tokens and terminal capability fallback
|-- motion.rs                EffectManager, EffectKey, recipes, reduced motion
|-- layout.rs                breakpoints, safe areas, table geometry
|-- terminal.rs              init/restore/panic/signal lifecycle
|-- screens/
|   |-- home.rs
|   |-- practice.rs
|   |-- host.rs
|   |-- host_lobby.rs
|   |-- join.rs
|   |-- public_lobby.rs
|   |-- table.rs
|   |-- study.rs
|   |-- replay.rs
|   |-- settings.rs
|   `-- help.rs
|-- components/
|   |-- app_frame.rs
|   |-- menu.rs
|   |-- form.rs
|   |-- dialog.rs
|   |-- toast.rs
|   |-- key_hints.rs
|   |-- card.rs
|   |-- seat.rs
|   |-- board.rs
|   |-- pot.rs
|   |-- action_bar.rs
|   |-- action_log.rs
|   |-- connection_badge.rs
|   `-- table_list.rs
`-- testing/
    |-- fixtures.rs
    |-- frame_assertions.rs
    `-- motion_clock.rs
```

This is a responsibility map, not a requirement to create every file before a
vertical slice needs it.

### Root route model

```text
Route
|-- Home
|-- PracticeSetup
|-- HostSetup
|-- HostLobby
|-- Join
|-- PublicLobby
|-- Table(TableContext)
|-- Study
|-- Replay(HandId)
|-- Settings
`-- Help(HelpContext)

Overlay
|-- None
|-- Dialog
|-- CommandPalette        optional later
|-- ToastStack
`-- ConnectionRecovery
```

An active practice authority, hosted authority, or joined session is activity
state, not a screen. Navigating to Help or Settings must not destroy it.

### Component contract

Each screen or interactive component should have four explicit concerns:

1. `handle_event` maps terminal/app events to UI messages or domain intentions.
2. `update` changes presentation/navigation state only.
3. `render` draws current state into a supplied `Rect` without blocking.
4. `presentation_events` request named effects after accepted state changes.

Do not make components open sockets, sleep, shuffle, apply poker actions, mutate
authoritative projections, or own wall-clock deadlines.

## Ratatui layout map

### Shared application frame

Every full screen receives:

- one-row brand/breadcrumb/connection header;
- a flexible content region;
- one-row contextual key-hint footer;
- overlay and toast layers rendered last;
- a minimum-size fallback that preserves terminal restoration and Help/Quit.

### At-table support boundary

Non-table routes may retain responsive menu/form constraints. At-table play has
only a support floor, not compact/standard/wide designs:

| Boundary | Size | At-table behavior |
|---|---|---|
| Unsupported | width below 56, height below 30, or below the matching width-band height | Minimum-size explanation; safe Help and Quit only; no partial table |
| Supported | >=80x30, >=72x32, >=64x36, or >=56x40 | One full-terminal component tree with continuous constraints and bounded truncation/scroll |

The primary design reference is 80x30. The mandatory comparison matrix is
56x40, 64x36, 72x32, 80x30, and 120x40. All five retain the same status,
portrait table stage, console, actions, and footer order. Extra width improves
breathing room, labels, and console line length; it never creates a wide side
rail or stretches the felt into a landscape table.

### Nine-handed ring geometry

Use stable perimeter anchors normalized around a vertically weighted stage:

```text
          S4      S5

      S3              S6
      S2  BOARD/POTS  S7
      S1              S8

            HERO S0
        ACTION CONTROLS
```

- Hero stays bottom-center so hole cards and actions do not move between table
  sizes.
- Other occupied seats distribute clockwise through perimeter anchors.
- Empty physical seats remain spatially stable for the configured table size;
  Practice, Host, and Join use the same anchor calculation.
- The acting seat gets a persistent non-motion highlight plus an optional short
  transition.
- Board, total pot, side-pot indicator, and action deadline remain central.
- Opponent seat cards are small facedown/revealed pairs; verbose status uses a
  focused-seat detail line or the persistent console, never a wide-only rail.
- The authoritative console stays in the same region at every supported size;
  only visible line count and truncation change.

### Screen composition map

| Screen | Main Ratatui components | Primary focus |
|---|---|---|
| Home | AppFrame, brand mark, Menu, Continue card, version/status | Four primary modes |
| Practice | Form, preset cards, seat/bot selector, summary dialog | Quick Practice first |
| Host | Form, visibility selector, seat map, reach warning | Safe configuration |
| Host Lobby | Invite card, seat list, waiting list, authority status | Share and start/wait |
| Join | Invite input, recent list, public-list entry, errors | One-value paste |
| Public Lobby | Filter bar, TableList, detail panel, join dialog | Discover and inspect |
| Table | Seat, Card, Board, Pot, ActionBar, ConnectionBadge, ActionLog | Legal action |
| Study | Hand list, filter bar, stats cards, learning menu | Recent hand |
| Replay | Table in read-only mode, timeline, action stepper, notes | Previous/next action |
| Settings | Tabs/form, theme preview, reduced-motion toggle | Safe local preferences |

## TachyonFX integration map

### Ownership

`UiEffects` owns one `EffectManager<EffectKey>`, an injectable presentation
clock, reduced-motion policy, and effect recipes. `EffectKey` identifies semantic
targets so repeated events replace obsolete effects rather than stack without
bound:

```text
Screen
Modal
Toast(id)
Seat(seat_id)
BoardStreet
Pot
ActionBar
Connection
Winner(seat_id)
```

TachyonFX effects are stateful, created once per presentation event, and applied
to the Ratatui buffer after stable widgets render. Unique keys cancel superseded
effects. Completed effects are removed by `EffectManager::process_effects`.

### Presentation-event catalogue

| Presentation event | Target | Candidate effect | Duration | Static truth retained |
|---|---|---|---:|---|
| App started | Screen | subtle `fade_from` or `sweep_in` | 180-240 ms | Home renders immediately |
| Route changed | content region | short directional sweep/fade | 120-180 ms | Route changes before effect |
| Modal opened | modal rect | fade/paint from overlay background | 100-140 ms | Modal focus is immediate |
| Toast added | toast rect | short `slide_in` plus fade | 120-180 ms | Message exists immediately |
| Hand started | table rect | restrained radial/diagonal reveal | 180-240 ms | New hand identity already applied |
| Hole cards dealt | hero card rects | staggered `slide_in` or `coalesce` | 100-160 ms each | Cards are already authorized |
| Street changed | board card rects | `coalesce`/fade by new card cells | 140-220 ms | Street and cards already applied |
| Actor changed | seat rect | one-shot lighten/paint | 100-160 ms | Persistent actor border/text remains |
| Action accepted | acting seat/log | brief color sweep | 100-150 ms | Revision and action text are final |
| Pot changed | pot rect | brief gold paint/sweep | 140-220 ms | Pot number changes immediately |
| Pot awarded | winner/pot | restrained parallel gold sweep | 260-400 ms | Award and stacks already reconcile |
| Connection lost | badge/overlay | short warning paint | 100-160 ms | Controls disable immediately |
| Reconnected | badge | green fade | 140-220 ms | Fresh snapshot is already applied |
| Error | relevant field/toast | brief red paint, no shake required | 100-160 ms | Error text and focus remain readable |

Durations are starting values for visual review, not poker timing policy.

### Never animate as authority

TachyonFX must never:

- delay or trigger an action, street, deal, showdown, award, timeout, reconnect,
  queue promotion, table start, or host drain;
- decide whether a control is legal or enabled;
- hide a changed authoritative value until an animation finishes;
- interpolate stacks, pots, wagers, contributions, revisions, or deadlines as if
  intermediate values were real;
- reveal unauthorized cards, including as particles, transition remnants, or an
  off-screen buffer;
- read deck order, random state, credentials, join codes, or internal table
  state;
- block input unless an ordinary modal/focus policy independently requires it.

If frames are skipped, the final authoritative screen must still be correct.

### Reduced motion

- Reduced motion is a first-class setting and can also be enabled by an
  environment/config accessibility preference.
- In reduced mode, navigation and game-state effects complete immediately or use
  a single low-intensity color change under 80 ms.
- No infinite shimmer, pulse, hue rotation, particle, dissolve, or idle table
  animation is required for comprehension.
- Connection and deadline status always have static text/symbol equivalents.

### Render scheduling

Use elapsed presentation time, not a fixed tick count:

1. Poll network, host, practice, and terminal events.
2. Apply accepted state changes.
3. Generate presentation events by comparing stable old/new view state.
4. Add/cancel semantic effects.
5. Render when state is dirty or `EffectManager::is_running()` is true.
6. Process effects against `frame.buffer_mut()` using elapsed time.
7. Target approximately 60 Hz while an effect runs and return to a lower-cost
   event/poll cadence when idle.

The existing 20 ms network poll is close to an animation cadence, but rendering
should become independently schedulable so idle screens do not redraw forever.

## Theme map

Move raw colors out of `render.rs` into semantic tokens:

```text
Theme
|-- surface.screen
|-- surface.panel
|-- surface.felt
|-- border.default / focused / actor / danger
|-- text.primary / secondary / muted / inverse
|-- accent.brand / gold / info / success / warning / danger
|-- card.face / back / red_suit / black_suit
|-- action.fold / check / call / raise / all_in
`-- overlay.scrim / panel
```

Required themes:

- default true-color theme;
- color-blind-safe theme;
- reduced-color 256-color fallback;
- monochrome fallback for `NO_COLOR` or incapable terminals.

Terminal capability detection chooses a palette, not a different information
hierarchy. The demo's inherited `NO_COLOR=1` incident should become a tested
fallback rather than an accidental loss of visual hierarchy.

## Player-facing table console

The table footer is a persistent, bounded player surface. It is not a debug log
and it does not use an interstitial hand-report screen. Dealer notices, blinds,
accepted actions, street and board transitions, pot changes, awards, stack and
session reconciliation, connection notices, and safe rejections remain visible
at the table. The latest four messages are shown at 120 x 40; a 200-message
session buffer is browsed with `PageUp`/`PageDown` and `Home`/`End`.

### At-table raise controls

The installed table has no modal raise mode. When betting or raising is legal,
the footer continuously shows 25%, 50%, 75%, pot, and 1.5x-pot presets, with
the legal minimum shown initially. Up and Down adjust that target by one chip;
`1` through `5` jump directly to the respective presets, and a subsequent arrow
adjustment returns the amount to a custom state. The action button shows the
resulting legal bet-to or raise-to amount. `R` submits that amount immediately
through the ordinary authoritative command path. Preset
targets use the pot after calling when facing a wager and clamp to the legal
minimum and maximum non-all-in target; `A` remains the explicit all-in action.

### At-table showdown sequence

The settled authoritative projection is presented in three deterministic
one-second stages before automatic rollover. Stage 1 marks every seat as shown,
folded, or mucked without leaking an unrevealed hand. Stage 2 highlights every
pot winner, including all players in a chop, and marks the exact five cards that
play using the shared evaluator over already-authorized cards and the public
board. Stage 3 shows each winner's exact aggregate chip award and settled stack.
The action console is temporarily replaced by this summary so later-stage
winner and award messages cannot spoil the reveal stage. This wall-clock
presentation state never controls settlement, stacks, pots, or tournament time.

Hero holdings include whitespace inside each bracket and between cards so
terminal glyph metrics cannot visually merge a suit with its boundary.

The design adapts established online-client patterns rather than copying their
artwork:

- [PokerStars table chat](https://www.pokerstars.com/help/articles/chat-feature-instructions/44737/)
  keeps player messages in a table Chat tab.
- [PokerStars hand histories](https://www.pokerstars.com/help/articles/save-hand-histories/239834/)
  exposes recent hands through the table chat window's Hands tab, while replay
  remains secondary.
- [PokerStars table appearance](https://www.pokerstars.com/help/articles/table-appearance-feature/232136/)
  distinguishes dealer, player, and observer message classes.
- [GGPoker table social features](https://legal.ggpoker.com/poker-games/table-social-features/)
  confirms that social interaction is attached to the live table rather than a
  post-hand report route.

The current tab is `DEALER`; `CHAT` is visibly reserved but disabled. A future
chat slice needs a separate bounded protocol, authenticated attribution,
rate-limiting, moderation/reporting, privacy and retention policy, and terminal
accessibility review. Dealer/system notices never enter that chat protocol.
Likewise, persisted hand histories remain an authorized Study concern rather
than being reconstructed from transient console text.

Only viewer-authorized projections and public events may generate console copy.
Wire direction, subscription state, command IDs, revisions, deterministic
seeds, authority internals, credentials, and unrevealed cards are forbidden.
This is enforced by negative tests and keeps operational diagnostics in their
separate allowlisted path.

## Testing and review map

### Deterministic tests

- Render the same at-table component tree at 56x40, 64x36, 72x32, 80x30, and
  120x40 with `TestBackend`; compare semantic landmarks, not only survival.
- Assert no panic and no non-space cell outside the target buffer.
- Snapshot semantic rows/regions, not only entire fragile buffers.
- Inject a motion clock and capture effect start, midpoint, completion, and
  reduced-motion completion.
- Prove unique effects cancel superseded seat/screen effects and the manager is
  empty after completion.
- Prove all effects are presentation-only by comparing authoritative state before
  and after arbitrary animation time advances.
- Add negative buffer tests for hidden cards, credentials, join codes, random
  state, and internal protocol diagnostics.

### Human visual review

- Capture from the installed `sneakyblinders` executable with a true-color
  terminal profile.
- Review Home, one form, lobby, two-seat table, nine-seat table, Study/replay,
  modal, reconnect, error, and reduced-motion states.
- For motion, capture a short frame strip or GIF for design review, while the
  mandatory sprint PDF retains still frames and one continuous hand.
- Visually inspect every source image and final PDF page under the existing
  review standard.
- Run a first-time-player usability session without exposing CLI flags.

### Performance acceptance

- Stable 60 Hz frame target during standard effects at the 80x30 primary
  reference and 120x40 continuity reference.
- No visible input lag or skipped authority update caused by effects.
- No unbounded effect accumulation during rapid network updates.
- No material regression to the existing 8-table/32-session backend profile.
- Ten repeated route/game cycles leave no effect, worker, socket, or terminal
  state behind.

## Reference intake

The first accepted reference set is catalogued in
UI Reference Catalogue (local archive: `../../assets/references/README.md`). It contains a
design-forward table, a typical nine-seat table/HUD, and a positional starting-
hand range chart with raise/shove/call/fold demarcation.

For each reference the product owner supplies, record:

1. source URL or attached-file provenance;
2. the exact screen or component it should influence;
3. what to borrow: hierarchy, geometry, typography, color, density, interaction,
   or motion;
4. what explicitly should not be copied;
5. expected 120 x 40 adaptation;
6. whether it is inspiration, a close target, or a hard acceptance reference;
7. any motion start/end frames or timing that matter.

Store approved local references under `docs/design/references/` with a small
index. Do not copy proprietary imagery or source code into the product. Translate
the visual principle into Ratatui-native geometry, text, symbols, and semantic
tokens.

The current product-owner-supplied files predate that convention and remain under
`assets/references/`; their catalogue is the index. Do not move or rename supplied
files without explicit approval.

### Applied direction from the first reference set

- Replace the 5/4 seat grid with a true perimeter composition.
- Anchor hero cards and large legal actions bottom-center.
- Keep board and pot as the dominant central focus.
- Separate seat identity from an optional, denser HUD layer.
- Keep live HUD off by default until its fairness policy is approved; any future
  values must come only from locally observed public actions and include sample
  size.
- Add a Study Range Explorer based on one legible 13 x 13 matrix at a time, with
  position, stack, action-sequence, source, and fold/raise/shove/call/mixed
  context.
- Adopt dark restrained surfaces, bright semantic action accents, and minimal
  presentation-safe motion as the provisional direction pending more references.

## Rebased next UI slice

The Home-to-production-table proof now exists, but it predates the shared UI
platform and is implemented outside a sprint acceptance boundary. The rebased
Sprint 14 goal is: **the installed `sneakyblinders` command runs a repeatable
Quick Practice journey through one cross-shell, semantic, accessible application
shell with presentation-safe motion.**

The UI-platform portion is refined as E12.9 inside the 40-point PX1 sprint:

| Story | Outcome | Points |
|---|---|---:|
| E12.9a | Ratatui/Crossterm/TachyonFX compatibility decision with the full existing gate | Part of 8 |
| E12.9b | Semantic theme, capability fallbacks, reduced motion, and deterministic presentation-only effects | Part of 8 |
| E12.1/E12.8 | Generalize the existing Home/table proof into the shared route, results, Help, and terminal lifecycle | Separate PX1 stories |
| E12.11a | Prove CMD, PowerShell, and Git Bash installed journeys | 3 |

The slice must continue to use a real projection-backed table. It cannot replace
authority with a fixture, claim Host/Join complete, or treat motion as game time.

## Planning consequence

The player-experience release rebase (local archive: `../agile/rituals/2026-09-01-player-experience-release-rebase.md`)
is complete. Sprint 14 accepted Shell/Quick Practice, Sprint 15 accepted private
Host/Join plus the functional single-table tournament, and Sprint 16 accepted
the one-renderer responsive portrait table. Study/productization, Custom
Practice, and D2 tournament breadth remain inactive backlog work.
