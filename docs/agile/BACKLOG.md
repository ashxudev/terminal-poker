# Product Backlog

Last reviewed: 2026-09-09. Sprint 20 completed 26/26: direct automatic LAN connection on TCP 6969.
Current forecast **759**, accepted **644**, remaining **115**. No active sprint.
LX-1/2/3 add 8 native Linux build/service/validation points for supplied hardware;
the existing 8-point E12.10 client packaging scope remains separate.
LB-1/2/3 add 21 explicitly refined local-lobby points. Sprint 18's 712/597/115
baseline and historical acceptance are preserved; no public Join points awarded.

## Product direction and order

The supplied Fedora 44 x86_64 ThinkPadServer is the current dedicated host.
The earlier Pi assumption is superseded for this milestone. Sprint 20 validates
direct verified TLS on port 6969; cross-VLAN checks beyond the development PC remain rollout work.
One server hosts multiple independent games. Game creators are clients; their
exit must not terminate the server. Practice remains local. One unified UI.

1. Reconcile requirements and source/release baseline.
2. Completed Sprint 18: separate server and game lifetime; installed Host/Join and proof.
3. Completed Sprint 19: remembered-server lobby, open/password games, installed evidence.
4. Completed Sprint 20: native Linux server build, service and remote validation.
5. Multiple-game/restart reliability and cross-VLAN rollout validation.
6. Custom Practice, then client packaging and direct VLAN validation.
7. Study histories/replay, statistics and notes.
8. Multi-table tournament expansion (D2).
9. Range Explorer and broader public release according to content/hosting decisions.

Usability, privacy, tests and recovery boundaries apply to each slice. Commit/push
and remote CI remain a separately authorized formal release gate; current work
preserves local source snapshots and does not infer permission to publish.

## Current allocation

| Work | Remaining points | State |
|---|---:|---|
| LB local game lobby | 0 | Sprint 19 accepted 21/21 |
| LX Linux dedicated host / LAN | 0 | Sprint 20 accepted 26/26: initial 8 plus LAN extension 18 |
| DS dedicated-server ownership | 0 | Sprint 18 accepted 13/13 |
| PX player experience | 47 | Practice 8; public Join 5; Study 13; ranges 8; packaging 8; final UX 5 |
| D2 multi-table tournaments | 34 | Inactive; separate from concurrent independent games |
| Public hardening | 34 | Transport 5; identity 8; operations 8; failure testing 8; candidate 5 |

The 18-point LAN extension is explicitly refined under ADR 0022. Remaining public
transport work covers trust distribution and internet deployment; implemented TLS
is reused without awarding its implementation twice.

## Current capability

Core E0-E4 and ring lifecycle are accepted locally. E5/E6/E7 authority, network
client and concurrent ring-table components have later normal-process evidence.
E8 scoped credentials are wired; recovery is between hands, not active-hand crash
durability. E9 D1 is accepted (55/89), D2 remains 34. E10/E11 have accepted local
hardening/candidate slices; public release is open. E12 is 74/121 accepted.
Sprint 17 added 26 accepted showdown points; current table protocol is v4; wire v5 and lobby v2 support listed passwords.

Latest installed build: game lobby, 314 tests passing, four ignored.
Earlier totals, acceptance boundaries and follow-up records are preserved in
pre-sprint archive (local archive: `rituals/2026-09-09-pre-sprint18/BACKLOG.md`).
Portfolio review (local archive: `rituals/2026-09-09-full-backlog-review.md`).

## Sprint 19

| ID | Story | Points | State |
|---|---|---:|---|
| LB-1 | Listed open/password access, privacy and checkpoint compatibility | 5 | Done |
| LB-2 | Remembered server, game directory, host/password and cancellation UI | 8 | Done |
| LB-3 | Process admission/isolation, installed journeys and inspected review | 8 | Done |

Open and password-protected games are both listed; password is enforced by the
server and never appears in listing/profile. New hidden games are deferred;
legacy Private/Unlisted saves remain hidden. No per-game IP/invite entry in the
main Join journey. First use configures the dedicated server; subsequent use
remembers it. Lobby is discovery on that server, not VLAN broadcast discovery.

## Sprint 18

| ID | Story | Points | State |
|---|---|---:|---|
| DS-1 | Backlog reconciliation and dedicated ownership contract | 3 | Done |
| DS-2 | Installed existing-server Host/Join, safe invites and cancellation | 5 | Done |
| DS-3 | Process lifecycle/isolation, installed journeys and inspected review | 5 | Done |

## Story catalogue - historical acceptance and remaining work

| ID | Story | Points | Dependencies | State |
|---|---|---:|---|---|
| E0.1 | Confirm ring-game and tournament release boundaries | 3 | Product owner | Done |
| E0.2a | Define poker rules policy catalogue and examples | 3 | Product owner/rules examples | Done |
| E0.2b | Approve seven configurable product defaults | 2 | Product owner | Sprint 12 - Accepted |
| E0.3 | Establish reproducible Rust toolchain and green baseline | 5 | Rust toolchain | Done |
| E0.4a | Decide server authority and one-authority-per-table execution | 3 | E0.1 | Done |
| E0.4b | Decide initial crate dependency direction | 2 | E0.4a | Done |
| E0.5 | Define capacity and recovery targets | 3 | Deployment intent | Sprint 12 - Accepted |
| E1.1a | Introduce neutral identifiers and eligible-seat traversal | 5 | E0.2a | Done |
| E1.1b | Migrate fixed state to seat-indexed collections | 8 | E1.1a | Done |
| E1.2 | Expand traversal examples across eligibility states | 5 | E1.1a | Done |
| E1.4 | Inject deterministic deck and random-source boundaries | 5 | E1.1b | Done |
| E1.5a | Preserve offline heads-up through named local seat bindings | 3 | E1.1b | Done |
| E1.5b | Route controllers through a common validated seat-command boundary | 5 | E1.5a | Done |
| E1.6 | Produce deterministic review-hand fixture and Sprint 2 PDF evidence | 5 | E1.4, E1.5b | Done |
| E2.1 | Calculate legal actions for any active seat | 8 | E1.5b | Done |
| E2.2 | Implement multiway betting-round completion | 8 | E2.1 | Done |
| E2.3 | Implement full-raise and short-all-in reopening rules | 13 | E2.2, RD-004 | Done |
| E2.4 | Build main and arbitrary side pots from contributions | 13 | E2.2 | Done |
| E2.5 | Resolve multiway showdowns, tied pots, and odd chips | 8 | E2.4, RD-005, RD-006 | Done |
| E2.6 | Add property tests for cards, turns, pots, and chip conservation | 5 | E2.1-E2.5 | Done |
| E4.1 | Define versioned command, event, snapshot, and error envelopes | 8 | E1.5b, ADR 0008 | Done |
| E4.2 | Project distinct views for each player and spectator | 13 | E4.1, ADR 0008 | Done |
| E4.3 | Add revisions, command IDs, acknowledgements, and rejection reasons | 8 | E4.1, ADR 0009 | Done |
| E4.4 | Add serialization, compatibility, malformed-message, and size-limit tests | 5 | E4.1-E4.3, ADR 0009 | Done |
| E5.1 | Run each table as one serialized state authority | 13 | E4.3-E4.4, ADR 0001, ADR 0009 | Done |
| E5.2 | Bind authenticated guest sessions to one authorized seat and hand | 8 | E5.1, E4.2, ADR 0010 | Done |
| E5.3 | Add deterministic server deadlines and automatic check/fold | 8 | E5.1, RD-007, ADR 0010 | Done |
| E5.4 | Route sessions to audience-specific table subscriptions | 8 | E5.1-E5.2, ADR 0010 | Done |
| E5.5 | Complete authorized-runtime integration tests with two through nine clients | 5 | E5.1-E5.4 | Done - in-process component |
| E6.2a | Build a projection-fed client model and responsive two-through-nine-seat geometry | 8 | E4.2, E5.4 | Done - component |
| E6.2b | Render authoritative network states through the actual Ratatui product path | 5 | E6.2a | Done - component; review fixture integration |
| E6.3 | Submit commands and reconcile only authoritative client outputs | 8 | E5.2-E5.4, E6.2a-E6.2b | Done - component reducer |
| E0.4c | Decide initial transport, framing, and bounded connection contract | 3 | E0.4a, E4.1 | Done |
| E6.1a | Preserve offline entry and add explicit server/connect CLI paths | 5 | E1.5a, E6.2a | Done |
| E6.3b | Wire the live terminal input/render loop to `ProjectionClient` | 8 | E6.2a-E6.3 | Done |
| E6.4b | Carry protocol v1 over independently running loopback processes | 8 | E0.4c, E4.1-E4.4, E5.5, E6.3 | Done |
| E6.6b | Prove transport disconnect and snapshot resynchronization | 5 | E6.4b, E6.3 | Done |
| E11.1a | Execute the single-table network-alpha candidate journey | 5 | E6.1a, E6.3b, E6.4b, E6.6b | Done |
| E1.7 | Consolidate offline and multiplayer rules paths or prove an intentional adapter boundary | 8 | E2, Release B candidate | Done |
| E3.1 | Join, reserve, occupy, and leave seats at safe boundaries | 8 | E1.7, seating policy | Done |
| E3.2a | Implement moving-button and ring entry policy | 8 | E0.2b, E3.1 | Sprint 12 - Accepted |
| E3.2b | Implement missed-blind debt and return boundaries | 5 | E0.2b, E3.2a, E3.3 | Sprint 12 - Accepted |
| E3.3 | Sit out, return, and leave after hand | 8 | E3.1 | Done |
| E3.4 | Start, pause, resume, and close tables based on eligibility | 5 | E3.1, E3.3 | Done |
| E7.4 | Register, locate, and route to many active table actors | 8 | E5.1, E3.4 | Done |
| E7.1 | Create, list, filter, and inspect public tables | 8 | E7.4 | Done |
| E6.4c | Wire the create/list/join CLI flow across real processes | 8 | E7.1, E7.4, E6.4b | Done |
| E11.1b | Execute the two-table ring candidate journey | 5 | E6.4c, E7.1, E7.4 | Done |
| E7.7 | Run repeated hands independently per table | 8 | E7.4, E3.4 | Sprint 11 - Accepted |
| E8.3a | Persist a validated between-hand registry checkpoint | 8 | E7.7 | Sprint 11 - Accepted |
| E8.4a | Restore tables without replaying an award | 8 | E8.3a | Sprint 11 - Accepted |
| E11.1c | Execute the repeated-hand restart candidate journey | 5 | E7.7, E8.3a, E8.4a | Sprint 11 - Accepted |
| E7.3a | Add bounded waiting and between-hand promotion | 8 | E3.1, E7.1 | Sprint 12 - Accepted |
| E10.3a | Expose structured table and recovery health | 5 | E7.4, E8.4a | Sprint 12 - Accepted |
| E7.6a | Exercise concurrent tables and mass reconnect at agreed local capacity | 8 | E0.5, E7.7, E10.3a | Sprint 12 - Accepted |
| E11.1d | Execute the graceful operational candidate journey | 5 | E7.3a, E7.6a, E10.3a | Sprint 12 - Accepted |
| E7.2 | Add private and unlisted tables | 5 | E0.2b, E7.1 | Sprint 12 - Accepted |
| E7.5 | Expire empty and inactive tables | 5 | E7.4, E10.3a | Sprint 12 - Accepted |
| E8.1a | Issue scoped expiring guest and reconnect credentials | 8 | E0.2b, E5.2, E6.6b | Sprint 12 - Accepted |
| E8.5a | Generate safe ring hand histories and mode-specific statistics | 8 | E4.2, E7.7 | Sprint 12 - Accepted |
| E6.5a | Complete multi-pot, showdown, history, and lifecycle presentation | 5 | E6.3b, E8.5a | Sprint 12 - Accepted |
| E11.6a | Run private-beta defect triage and burn-down | 3 | Expanded Sprint 12 functional stories | Sprint 12 - Accepted |
| E11.5a | Publish the private-beta local playtest/operator quickstart | 1 | E10.3a, E11.1d | Sprint 12 - Accepted |
| E11.1e | Execute the private ring-game beta candidate | 5 | Expanded Sprint 12 functional stories | Sprint 12 - Accepted |
| E10.1a | Remove recoverable access and route identity material from durable state | 5 | E7.2, E8.3a | Sprint 13 - Accepted |
| E10.2a | Wire scoped guest/reconnect credentials end to end | 8 | E10.1a, Sprint 12 credential primitives, E6.4b | Sprint 13 - Accepted |
| E10.6a | Add signal-driven drain and recovery diagnostics | 5 | E11.1d, E10.3a | Sprint 13 - Accepted |
| E10.3b | Persist bounded safe ring histories | 8 | E8.5a, E8.3a | Sprint 13 - Accepted |
| E10.4a | Run a two-hour private-beta soak with reconnect waves and alerts | 8 | E7.6a, E10.3a | Sprint 13 - Accepted |
| E11.1f | Execute the durable private-beta candidate | 5 | Proposed Sprint 13 hardening stories | Sprint 13 - Accepted |
| E12.1 | Build the installed application shell, route/event model, Home, and terminal-safe lifecycle | 8 | E6.3b, player-experience requirements | Sprint 14 accepted; unified reducer, key-release guard, RAII/panic restoration, failure/interrupt probes, and installed no-flag shell proven |
| E12.9 | Establish compatible Ratatui/Crossterm/TachyonFX, semantic themes, capability fallbacks, and reduced motion | 8 | E12.1, UI map | Sprint 14 accepted; preferred 0.30.2/0.29/0.25.1 stack, bounded effects, and restricted-color conversion proven |
| E12.2 | Add versioned local profile, settings, data layout, and migrations | 5 | E12.1, data policy | Sprint 14 accepted; installed restart, concurrent atomic publication, migration, and corruption preservation proven |
| E12.3a | Prove one projection-safe nine-handed Quick Practice hand through the installed route | 3 | E12.1, E5.5 | Sprint 14 accepted; installed authoritative route, projection privacy, negative secret scan, cross-shell terminal outcome, and 900-chip conservation proven |
| E12.3b | Add repeatable Quick Practice, table-console outcomes, and automatic next-hand lifecycle | 5 | E12.3a, E12.2 | Sprint 14 accepted; installed three-hand CMD journey and authority lifecycle preserve stacks, rotate button, print reconciled outcomes, and retain safe histories |
| E12.8 | Share table shell, player-facing console, contextual Help, and clean mode transitions | 8 | E12.1, E12.3b, E12.9 | Sprint 14 accepted; unified reducer/rendering, bounded non-diagnostic table console, compact/error states, bounded motion, and ten-cycle lifecycle regressions proven |
| E12.8b | Replace the production breakpoint renderer swap with one full-terminal portrait-first responsive table | 8 | E12.8, E12.9 | Done - Sprint 16 accepted; one production composition spans the approved 80x30, 72x32, 64x36, and 56x40 support staircase through large viewports |
| E12.11a | Prove the installed journey across CMD, PowerShell, Git Bash, viewports, and capability fallbacks | 3 | E12.1-E12.3b, E12.8-E12.9 | Sprint 14 accepted; installed cross-shell full journeys, final-candidate smokes, viewport/capability/failure matrix, and restoration evidence pass |
| E12.11c | Prove the installed single table across minimum/primary/large viewports, live resize, shells, capabilities, visual consistency, and human review | 5 | E12.8b, E12.11a | Done - Sprint 16 accepted; deterministic Ratatui viewport/hand evidence, cross-shell installed smokes, privacy/conservation checks, and an eight-page visually inspected PDF pass |
| E12.3c | Add Custom Practice for two to nine seats with bounded bot profiles | 8 | E12.3b | Deferred until dedicated-server lifecycle is established; bot scope requires refinement |
| E12.4a | Integrate This-computer Host authority supervision, health, checkpoint, and safe drain | 8 | E12.1, E7, E8, host policy | Sprint 15 accepted |
| E12.4b | Add the Host lobby, structure/registration UI, and opaque invite | 5 | E12.4a, E12.1, invite policy | Sprint 15 accepted |
| E12.5a | Integrate private-invite/recent Join, waiting, credentials, and reconnect UX | 8 | E12.4a-E12.4b, E7, E8, invite policy | Sprint 15 accepted; This-computer private-invite path only |
| E12.5b | Add public discovery and online Join reach | 5 | E12.5a, public identity/transport policy | Deferred; private LAN connection milestone precedes public discovery |
| E12.6 | Build Study with authorized histories, replay, filters, statistics, notes, and Learn | 13 | E8.5a, E10.3b, E12.1 | Backlog; split before activation |
| E12.7 | Build Range Explorer schema, provenance, validation, and synthetic-fixture UI | 8 | E12.6, content policy | Content decision required |
| E12.10 | Deliver single-artifact packaging, atomic update/uninstall, data preservation, and diagnostics | 8 | E12.1-E12.9 | Backlog |
| E12.11b | Iterate control discoverability, then run first-time usability, accessibility, installed E2E, and final PX release evidence | 5 | E12.3c-E12.10 | Controls implemented in follow-ups; complete first-time usability/accessibility and PX evidence remain |
| E9.0 | Approve the functional single-table tournament policy and configuration contract | 3 | Product owner, rules examples | Sprint 15 accepted |
| E9.1a | Build bounded Host tournament setup for entrants/table size, starting stack, starting blinds/antes, level timer/schedule, breaks, and play-money payouts | 5 | E9.0, E7, E8 | Sprint 15 accepted; configuration is editable before lock and server-owned after start |
| E9.1b | Add bounded idempotent registration, configuration lock, under-minimum cancellation, and setup summary | 3 | E9.1a, E7, E8 | Sprint 15 accepted |
| E9.2a | Assign and start one two-to-nine-player tournament table | 5 | E9.1b, E3, E5 | Sprint 15 accepted |
| E9.2b | Integrate the installed tournament Host/Join setup, preview, registration, and table journey | 5 | E12.4a-E12.5a, E9.1a-E9.2a | Sprint 15 accepted |
| E9.3a | Add the authoritative monotonic tournament clock, visible level countdown, and first level | 5 | E9.0, E9.2a | Sprint 15 accepted |
| E11.1g | Execute the single-table tournament-start candidate | 3 | E9.1a-E9.3a, E9.2b | Sprint 15 accepted |
| E9.3b | Apply levels, antes, and scheduled breaks at safe boundaries | 8 | E9.3a | Sprint 15 accepted |
| E9.6 | Track exact-once bust-outs, winner, deterministic standings, and configured play-money payouts | 5 | E9.2a, E9.3b | Sprint 15 accepted |
| E9.8a | Recover one tournament and table from a validated between-hand boundary | 8 | E9.3b, E9.6, E8 | Sprint 15 accepted |
| E11.1i | Execute the functional single-table tournament candidate | 5 | E9.1a-E9.8a | Sprint 15 accepted; product owner reported no functional complaints and would play again |
| E9.4a | Select deterministic multi-table balance moves | 8 | Functional single-table tournament | Post-target D2 (inactive) |
| E9.4b | Commit atomic cross-table player moves and route handoff | 8 | E9.4a | Post-target D2 (inactive) |
| E9.5 | Break tables and route every survivor safely | 5 | E9.4b | Post-target D2 (inactive) |
| E9.7a | Consolidate survivors to a final table | 5 | E9.5 | Post-target D2 (inactive) |
| E9.7b | Coordinate configured hand-for-hand play | 5 | E9.3b, E9.4b | Post-target D2 (inactive) |
| E11.1h | Execute the multi-table tournament expansion candidate | 3 | E9.4a-E9.7b | Post-target D2 (inactive) |

## Discovered work

Add newly discovered work here before assigning it to an epic.

| ID | Description | Source | Proposed epic | State |
|---|---|---|---|---|
| DISC-001 | Decide whether one terminal client may play multiple tables | Requirements review | E6/E7 | Backlog |
| DISC-002 | Decide public/private bot policy | Requirements review | E1/E7 | Backlog |
| DISC-003 | Normal executable remained on the legacy heads-up engine while network UI evidence used review fixtures | 2026-08-31 release rebase | E1/E6 | Converted to E6.3b and E1.7 |
| DISC-004 | Multi-table acceptance server completes configured hands but lacks a persistent per-table rollover loop | Sprint 10 review | E7 | Converted to E7.7 |
| DISC-005 | Replace fragmented player commands with one installed `sneakyblinders` application shell for Practice, Host, Join, Study, Settings, and Help | Player experience requirements | E12 | Converted to E12.1-E12.11b; Home/Quick PX1 slice accepted in Sprint 14 |
| DISC-006 | Port practice bots through the multiway authority for one-human, 1-8-bot play while preserving authorized projections | Player experience requirements | E12 | Converted to E12.3a-E12.3c |
| DISC-007 | Define a single opaque invite format and progressive This-computer/LAN/Online hosting reach without exposing credentials or unsafe transport | Player experience requirements | E12/E10 | Private-invite path converted to E12.4a-E12.5a for Sprint 15; public discovery/online reach remains E12.5b after D1 |
| DISC-008 | Build player-authorized local history, replay, notes, mode-specific statistics, and Learn surfaces | Player experience requirements | E12 | Converted to E12.6 |
| DISC-009 | Upgrade the shared Ratatui/Crossterm/TachyonFX buffer stack and establish semantic theme, deterministic motion, and reduced-motion behavior | Ratatui/TachyonFX UI map | E12 | Converted to E12.9; perimeter geometry already has a production slice |
| DISC-010 | Decide whether a live human-table HUD is allowed and constrain it to locally observed public actions with sample-size disclosure | Product-owner table/HUD reference | E12 | Product/fairness decision required; default off |
| DISC-011 | Define versioned range-strategy content provenance and validation independently from the 13 x 13 Study renderer | Product-owner range-chart reference | E12 | Converted to E12.7; synthetic fixtures allowed |
| DISC-012 | Git Bash/ConPTY can emit a launch-key release that a naÃ¯ve input loop treats as a fresh menu action | Installed-shell human test | E12 | Specific defect fixed; cross-shell matrix refined as E12.11a |

| DISC-013 | Refine the parallel policy-learning programme beginning with an independently validated mathematical oracle while preserving ADR 0016 | Policy-harness reassessment | Separate proposed AI epic | Initial deal, observation, action, arena, CLI, benchmark, and conformance foundation implemented outside sprint; next slice is unestimated and inactive |
| DISC-014 | Improve gameplay-control discoverability without regressing keyboard efficiency or terminal compactness | Sprint 14 product-owner retest | E12 | Folded into E12.11b and the acceptance criteria of every new PX2/PX3 journey; current build is functional and replayable, not yet intuitive |
| DISC-015 | Give the tournament host bounded pre-start control of table capacity, starting stack, starting blinds/antes, level timing/schedule, breaks, and play-money payout structure | Post-Sprint-14 product direction | E9 | Converted to E9.0, E9.1a-E9.1b, E9.2b, E9.3a, and E9.6 without changing the existing 55-point single-table allocation; exact limits/presets remain refinement work |
| DISC-016 | Remove the tournament starting-stack minimum expressed as a number of big blinds while retaining absolute chip, arithmetic, and protocol safety bounds | Sprint 15 product-owner review | E9 | Ready for refinement; do not silently remove absolute validation bounds |
| DISC-017 | Converge compact and large table layouts around one smaller portrait-first composition instead of materially different compact/landscape variants | Sprint 15 product-owner review | E12 | Resolved by Sprint 16 through E12.8b/E12.11c and D-036; the approved support staircase bottoms at 56x40 and preserves one portrait composition |
| DISC-018 | Separate holding brackets from suit glyphs and replace the modal raise-confirm flow with directly adjustable sizing, preset hotkeys, and immediate R submission | Post-Sprint-16 product-owner feedback | E12 | Implemented outside a sprint without points: padded holdings; Up/Down one-chip adjustment; 1-5 hotkeys for 25/50/75/pot/1.5x-pot targets; immediate R bet/raise; focused/full gates, real-Ratatui visual inspection, exact installation, and cross-shell smokes pass; human retest pending |
| DISC-019 | Replace the sub-second terminal hold with staged showdown, winner, and award presentation; distinguish folds/mucks and show each winner's playing five and payout | Post-Sprint-16 product-owner feedback | E12 | Implemented outside a sprint without points; full gate, six-frame real-Ratatui visual inspection, exact install, and cross-shell smokes pass; human retest pending |
| DISC-020 | Include all kickers in complete-hand comparisons so best-five presentation and authoritative pot winners agree with poker ranking | Installed Quick Practice on 2026-09-07 | E2/E12 | Fixed in source outside a sprint without points; regression coverage includes the played hand, all four affected categories, 2-9-seat main/side pots, genuine board ties, and standard/minimum production rendering. See defect record (local archive: `rituals/2026-09-07-kicker-comparison-fix.md`) for validation and installation. |

The policy-learning programme is excluded from the 660-point product baseline,
the 558 accepted points, and the 115-point remaining roadmap. Environment
throughput is not policy quality, and no retrospective points are assigned to the
implemented foundation.

### Card-presence follow-up

DISC-021 (2026-09-07): opponent card presence and in-hand fold visibility fixed
and installed outside a sprint without points. Shared production panels now
distinguish concealed live/all-in hands, folds, not-dealt seats, and empty
seats; folded hero cards no longer look live. Privacy and five-viewport
regressions pass. Screenshot/validation record (local archive: `../../output/card-presence/README.md`).

### Showdown pacing follow-up

DISC-022 (2026-09-07): requested showdown pacing installed outside a sprint
without points: 1.5-second reveal, 1.5-second winner highlight, existing
one-second award. Bright green brackets identify winning holdings and best-five
cards, including ties. Full quality gate and screenshot/shell checks pass;
human retest pending. Evidence (local archive: `../../output/showdown-timing/README.md`).

### Heart suit follow-up

DISC-023 (2026-09-07): replace the heavy heart U+2764 with the playing-card
heart U+2665, retaining text presentation, to match the other suit glyphs in
CMD. One-line shared symbol fix; no sprint or points added.

## Icebox

- Durable user accounts and password recovery
- One user playing multiple tables simultaneously
- Spectator mode
- Chat and moderation
- Re-entry and rebuy tournaments
- Multiple deployment regions
- Graphical, web, and mobile clients
- Provably fair commit/reveal shuffling
- Real-money wagering, which requires a separate regulated programme

## Unmapped requirements requiring refinement

- Practice durable save/resume/pause and session-end restart behavior.
- Distinct bot styles; learned policy remains outside the product baseline.
- Active-hand recovery versus between-hand public RPO (PD-006/PD-010).
- DISC-016 removal of the tournament 20-BB floor, retaining arithmetic bounds.
- Source checkpoint/remote CI and service operation ownership.

Public hardening catalogue: E10.1b 5, E10.2b 8, E10.6b 8, E11.2 8,
E11.1j 5. Scope remains as recorded in the prior remaining-milestones plan.

Follow-up for connection UX: registration waits and reconnect retry loops need
responsive cancellation/status rather than synchronous waiting; refine with LAN
connectivity work. Existing waiting behavior is not changed by Sprint 18.

### Sprint 19 follow-through

21/21 accepted with inspected review and final installed three-shell replay.
No new product sprint activated. LAN refinement must include protected transport,
server-address/DNS policy, responsive connection operations and admission rate
limits before exposure. Keep existing public transport/Join allocations explicit
and re-estimate that scope before activation; this lobby does not close them.

Focused follow-up: lobby UI alignment completed and installed, with shared shell
styling across directory, setup, password, waiting and results. Full gate and
three-shell installed smoke passed; no extra points or backlog reordering.
See verification (local archive: `rituals/2026-09-09-lobby-ui-alignment.md`).

Sprint 20 extension adds 18 private-LAN points under ADR 0022. Public hardening
remains distinct: internet trust/discovery, account identity and public operations.

Corrective follow-up 2026-09-09: waiting-host admission/TLS closure defect fixed
and deployed. Added shared-IP and delayed-join regressions. No new points or
reordering; see rituals/2026-09-09-waiting-host-fix.md.

Release-preparation audit: exact proposed source checkout and fork-PR route
recorded in ../development/PR_TRACKING_AUDIT.md. Source isolation passes; macOS CI,
portable onboarding and final staged-diff review remain before PR readiness.
No new sprint or points; existing packaging/release allocation unchanged.
