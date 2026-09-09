# Decision Register

Last reviewed: 2026-09-08

Material architecture decisions belong in `docs/adr/`. This register tracks product, rules, and architectural decisions together.

## Accepted

D-040: integrate the reviewed bitmap main menu into the existing ShellApp, with
embedded assets, detected terminal graphics, cached rendering, six shared routes,
and accessible text fallback. [ADR 0018](../adr/0018-embedded-branded-home.md).

Current D-039: user-requested PokerStars alignment supersedes D-038's reveal order
and human decision window. Checked rivers start left of the button; called river
aggressors show first; beaten hands automatically muck for all seats. H remains
a pre-showdown always-show preference. Reveal/winner pacing is retained without
muck-decision delays. Protocol/wire v4; public muck privacy is unchanged.
[ADR 0017](../adr/0017-ordered-tournament-showdown.md).

Historical decision follows.

Post-review D-038: the user explicitly selects best-hand-first on checked rivers,
a shared five-second H-to-show window for losing humans, immediate AI auto-muck,
and green brackets only on cards in the best five. Uncontested hands never run
out. This digital house policy supersedes the corresponding D-037 behavior;
[ADR 0017](../adr/0017-ordered-tournament-showdown.md) records the boundary.
Protocol/wire v3 is required for the new optional-choice projection and command
semantics. Historical v2 evidence below remains tied to the Sprint 17 build.



Sprint 17 decision D-037: [ADR 0017](../adr/0017-ordered-tournament-showdown.md)
defines ordered automatic show/muck, a per-hand show override, mandatory all-in
exposure before runout, and private histories. The serialized authority advances
the flow on a server clock, with protocol/wire v2 rejecting older peers. The
user's prior explicit bright-green bracket request supersedes D-036's felt-only
green rule for winning holdings and best-five highlights.

| ID | Decision | Rationale | Date | Record |
|---|---|---|---|---|
| D-001 | Support two to nine players per table | Product clarification | 2026-08-30 | Requirements |
| D-002 | Support many concurrent tables | Product clarification | 2026-08-30 | Requirements |
| D-003 | Include multi-table tournament capability | Product clarification | 2026-08-30 | Requirements |
| D-004 | Treat chips as play-money only | Avoid regulated real-money scope | 2026-08-30 | Requirements |
| D-005 | Use a server-authoritative game model | Required for correctness, privacy, and recovery | 2026-08-30 | [ADR 0001](../adr/0001-server-authority-and-table-actor.md) |
| D-006 | Deliver the table/ring milestones before tournament orchestration while retaining tournaments in the target | Tournaments consume the generalized table engine | 2026-08-30 | Product kickoff |
| D-007 | Keep one package during core migration with dependencies pointing inward; extract crates after stable consumers exist | Avoid premature workspace churn while preserving a pure-domain boundary | 2026-08-30 | [ADR 0002](../adr/0002-module-and-crate-dependency-direction.md) |
| D-008 | Inject authoritative shuffle sources; use entropy-seeded production construction and explicit deterministic review fixtures | Preserve fairness authority while enabling reproducible tests and reports | 2026-08-30 | [ADR 0005](../adr/0005-authoritative-randomness-and-deterministic-review.md) |
| D-009 | Route every controller through one validated, mutation-free-on-rejection seat-command boundary | Make local, bot, and future network intent equally untrusted and auditable | 2026-08-30 | [ADR 0006](../adr/0006-validated-seat-command-boundary.md) |
| D-010 | Use cumulative full-raise reopening, contribution-layered pots, button-clockwise odd chips, and required-hand showdown reveal | Make multiway action rights and independent pot awards deterministic | 2026-08-30 | [ADR 0007](../adr/0007-multiway-betting-pots-and-showdown.md) |
| D-011 | Use versioned JSON-compatible envelopes and construct separate player/spectator projections at an explicit privacy boundary | Establish stable stale-command semantics without serializing internal table state | 2026-08-30 | [ADR 0008](../adr/0008-versioned-envelopes-and-private-projections.md) |
| D-012 | Retain exact command outcomes in a bounded no-eviction per-hand ledger and place authority behind one bounded mailbox worker | Make retries at-most-once and within-table transitions race-free before transport work | 2026-08-31 | [ADR 0009](../adr/0009-bounded-idempotency-ledger-and-table-mailbox.md) |
| D-013 | Derive seat and audience from in-worker guest bindings, own logical deadlines in the actor, and publish bounded ordered private subscriptions | Prove authorization, time, and routing before choosing transport | 2026-08-31 | [ADR 0010](../adr/0010-authorized-sessions-deterministic-deadlines-and-subscriptions.md) |
| D-014 | Preserve component-story acceptance but require normal executable and independent-process evidence for playable release gates | Prevent review fixtures from being mistaken for a launchable multiplayer product | 2026-08-31 | Release rebase (local archive: `rituals/2026-08-31-release-rebase.md`) |
| D-015 | Complete the single-table executable gate, then consolidate the engine/table lifecycle before lobby breadth, and retain tournaments after the multi-table ring milestone | Convert proven foundations into vertical player journeys without weakening the agreed tournament vision | 2026-08-31 | Release rebase (local archive: `rituals/2026-08-31-release-rebase.md`) |
| D-016 | Use bounded length-prefixed JSON over loopback TCP for the local Release B candidate, with server-prebound guest sessions and fresh-snapshot reconnect | Cross a real byte/process boundary without changing protocol v1 or prematurely selecting public TLS/WebSocket topology | 2026-08-31 | [ADR 0011](../adr/0011-bounded-loopback-tcp-json-framing.md) |
| D-017 | Use `MultiwayHand` as the sole server rules authority, retain `GameState` only as a heads-up offline compatibility adapter, and isolate roster changes behind snapshot/reconcile boundaries | Prevent dual-engine drift and active-hand corruption without forcing unrelated offline UX migration | 2026-08-31 | [ADR 0012](../adr/0012-authoritative-rules-and-table-lifecycle-boundary.md) |
| D-018 | Keep bots in offline and private-table adapters only until a later public-table policy explicitly changes this | Preserve training/private use without silently populating public tables | 2026-08-31 | Sprint 9 activation policy |
| D-019 | Route public-table sessions through one bounded process-local registry, construct public lobby summaries from an explicit allowlist, and preserve one serialized authority per running table | Add the second routing dimension without exposing hand/session state or combining it with persistence and distributed coordination | 2026-08-31 | [ADR 0013](../adr/0013-bounded-multi-table-registry-and-public-lobby.md) |
| D-020 | Persist only a bounded, checksummed between-hand registry allowlist and construct fresh monotonic authorities after whole-document validation | Recover rosters and stacks without serializing hidden/live state or risking duplicate terminal awards | 2026-08-31 | [ADR 0014](../adr/0014-between-hand-checkpoint-and-restart-boundary.md) |
| D-021 | Expand the inactive Sprint 12 recommendation to combine the 29-point operable loop and 55-point private ring-game beta under a 1,200,000-token target and 1,900,000 ceiling | Complete one coherent ring milestone with a shared gate/review while retaining explicit defaults approval, token checkpoints, and tournament/public-hardening exclusions | 2026-08-31 | Sprint 12 scope expansion (local archive: `rituals/2026-08-31-sprint-12-scope-expansion.md`) |
| D-022 | Accept the seven ring-game beta defaults: moving button; wait-or-post-live-BB entry; live-BB plus dead-SB debt when both are owed; clockwise odd chips; tournament-aligned reveal/muck; constant 60-second reconnect grace; check-otherwise-fold timeout | Remove the long-standing E0.2b ambiguity with executable behavior while leaving tournament-specific policy for E9 | 2026-08-31 | [Policy catalogue](../rules/POLICY_CATALOGUE.md) |
| D-023 | Use one terminal/table per guest during private beta; support public, unlisted, and private tables; allow bots only offline or at private tables; require between-hand restart recovery but not mid-hand replay | Bound the controlled beta without prematurely adding accounts, public bots, multi-tabling, or active-hand durability | 2026-08-31 | Sprint 12 planning (local archive: `rituals/2026-08-31-sprint-12-planning.md`) |
| D-024 | Harden normal-path identity, recoverable access/route material, signal drain, durable safe histories, and sustained soak before tournament orchestration; preserve the 162-point Sprints 13-17 partition | The post-Sprint-12 executable is a genuine controlled local beta, but its concrete security/operations gaps should not be copied into a new tournament controller | 2026-08-31 | Post-Sprint-12 release rebase (local archive: `rituals/2026-08-31-release-rebase-after-sprint-12.md`) |
| D-025 | Use random stable principals, digest-backed scoped rotating reconnect capabilities, verifier-only private access, an independent bounded safe-history store, and one signal-driven drain boundary | Remove recoverable authority from durable state while preserving between-hand recovery, exact-route ownership, optional history recovery, and controlled local play | 2026-09-01 | [ADR 0015](../adr/0015-durable-private-beta-credentials-history-and-drain.md) |
| D-026 | Permit validated complete deal plans only through an explicitly training-named crate-private deck seam, while policies remain restricted to private projections, public events, and authoritative commands | Enable exact ranges, runout branches, and duplicate evaluation without weakening production shuffle authority or exposing future/private cards | 2026-09-01 | [ADR 0016](../adr/0016-training-only-deal-plans-and-policy-boundary.md) |
| D-027 | Insert a 108-point installed-player-experience milestone before tournament breadth, reforecast the product backlog from 552 to 660, and retain `sneakyblinders` as the one working player command | Human feedback showed that proven platform components are not yet a coherent product; complete Practice, Host, Join, Study, packaging, and usability before multiplying tournament UI breadth | 2026-09-01 | Player-experience release rebase (local archive: `rituals/2026-09-01-player-experience-release-rebase.md`) |
| D-028 | Preserve the installed-shell tangent as implemented but accept zero retrospective points until an activated sprint completes the installed cross-shell, full-gate, PDF, visual, and human-review boundaries | Avoid losing useful software or distorting throughput/acceptance by retroactively treating an unbudgeted tangent as a completed sprint | 2026-09-01 | Tangent retrospective (local archive: `rituals/2026-09-01-installed-shell-tangent-retrospective.md`) |
| D-029 | Keep the policy-learning harness as a separate, unestimated research programme outside the 660-point product baseline and Sprints 14-20; require separate promotion and deployment authority for any learned bot | Prevent deterministic environment throughput or training artifacts from being mistaken for policy quality, release progress, or permission to expose a bot to players | 2026-09-01 | Player-experience release rebase (local archive: `rituals/2026-09-01-player-experience-release-rebase.md`) |
| D-030 | Make a functional two-to-nine-player networked single-table freezeout the next target after PX1/PX2; split the existing 89 tournament points into a 55-point target and 34-point post-target multi-table expansion | Deliver registration-to-winner through the installed product before taking on balancing, atomic movement, breaking, consolidation, and hand-for-hand coordination, without abandoning the long-term multi-table vision or inflating the 660-point baseline | 2026-09-01 | Functional single-table tournament rebase (local archive: `rituals/2026-09-01-single-table-tournament-release-rebase.md`) |
| D-031 | Keep hand outcomes and safe public game-state changes in a persistent player-facing table console; remove the interstitial Results route and reserve separately governed channels for future chat and hand history | Established online clients keep dealer/chat/history at the table, while the product owner rejected a blocking report screen; continuity, notifications, and future social interaction belong in one bounded table surface without exposing protocol diagnostics | 2026-09-01 | [UI map](../development/RATATUI_TACHYONFX_UI_MAP.md#player-facing-table-console) |
| D-032 | Make the first tournament Host journey a bounded pre-start structure editor for entrant/table capacity, starting stacks, blinds/antes, level timing/schedule, breaks, and play-money payout/results; lock one validated server-owned configuration at start | The product owner wants direct tournament-structure control, while server authority must still prevent a host from mutating cards, pots, live stacks, action rights, eliminations, or awards | 2026-09-02 | [Tournament requirements](../../NETWORKED_MULTIPLAYER_REQUIREMENTS.md#tournament-configuration) |
| D-033 | Make the complete functional single-table tournament milestone the Sprint 15 outcome under a 750,000-token runtime ceiling; combine 21 tournament-critical Host/Join points with all 55 D1 points and move Custom Practice plus public Join/reach behind D1 | The product owner requires the milestone next sprint. Custom Practice and public reach are not dependencies of a two-to-nine-human private-invite Host/Join freezeout, so critical-path sequencing reduces the commitment from 89 to 76 without weakening D1 | 2026-09-02 | Sprint 15 recommendation (local archive: `rituals/2026-09-02-sprint-15-functional-single-table-tournament-recommendation.md`) |
| D-034 | Accept the Sprint 15 fixed-start private tournament defaults: this-computer reach; one opaque invite; two-to-nine humans; locked registration; bounded stacks, blinds, antes, levels and breaks; moving button; between-hand schedule changes; deterministic same-hand elimination order; basis-point play-money payouts with largest-remainder allocation; between-hand recovery only | The product owner activated the recommendation without replacing its enumerated defaults, turning the policy/configuration questions into executable acceptance contracts while retaining public reach, bots, late registration, rebuy, multi-table movement, hand-for-hand, and active-hand replay as exclusions | 2026-09-02 | Sprint 15 recommendation (local archive: `rituals/2026-09-02-sprint-15-functional-single-table-tournament-recommendation.md`) |
| D-035 | Use one responsive player-facing table renderer for Practice, Host, and Join; preserve one component order and seat grammar through resize; make the table portrait-first and prevent extra width from stretching it into a landscape design | The product owner rejected materially different compact and large variants and wants one UI sized to the terminal with a small supported minimum | 2026-09-02 | Post-Sprint-15 table UI refinement (local archive: `rituals/2026-09-02-post-sprint-15-single-table-ui-refinement.md`) |
| D-036 | Adopt Variant 01 Portrait Oval and a width/height support envelope of 80x30, 72x32, 64x36, and 56x40; reserve green for felt, use dark monochrome chrome, and use red for action highlights | Product-owner approval after deterministic mockup comparison and narrow-terminal proof | 2026-09-02 | Sprint 16 (local archive: `rituals/2026-09-02-sprint-16-responsive-portrait-table.md`) |

## Dedicated-server direction - 2026-09-09

D-042: one separately running server owns multiple independent games. Creators
are clients. Develop locally without Pi hardware; retain loopback-only transport
until a protected LAN milestone. ADR 0019 governs Sprint 18. Unified appearance
supersedes older theme/motion requirements. D2/public sprint numbers below are
historical, not active reservations. PD-006/PD-010 recovery remains unresolved.

## Pending product decisions

| ID | Question | Default assumption | Needed by | Owner |
|---|---|---|---|---|
| PD-002 | May one user play multiple tables simultaneously after private beta? | Private beta accepts one table per terminal client | Before durable multi-table user identity | Product owner |
| PD-003 | Are guest identities sufficient for public launch? | Private beta accepts scoped guest sessions; public launch remains undecided | Before public release | Product owner |
| PD-006 | Must active games survive process restart at alpha? | Required for public beta, not first local alpha | Before E8 | Product/operations |
| PD-010 | What identity, hosting, capacity, RPO/RTO, and operational ownership govern public release? | Scoped guests, one region/node at measured capacity, between-hand RPO, and named operator/support owner | Before rebased Sprint 18 | Product/operations |
| PD-012 | What HUD and range-content policy governs the first Study journey? | Live HUD off; Study/Practice may use authorized data; Range Explorer ships synthetic fixtures until an attributable versioned content pack is approved | Before Study/Range activation | Product owner/content owner |
| PD-013 | What is the accepted minimum for the one responsive at-table UI? | Resolved by D-036: use the approved width/height envelope through the 56x40 stress case rather than one fixed 80x24 threshold | Resolved 2026-09-02 | Product owner |

## Pending poker-policy decisions

Ring-game policies RD-001, RD-002, RD-003, and RD-007 are accepted in D-022.
Single-table configuration bounds/presets, schedule-boundary, break, payout
rounding, and tie-order defaults are recommended for acceptance or replacement
at Sprint 15 activation. Cross-table movement-near-blinds and hand-for-hand
policy are explicitly deferred until the post-target Sprint 17 expansion.

## Pending architecture decisions

| ID | Decision needed | Candidate default | ADR |
|---|---|---|---|
| AD-001 | Rules/server/client crate boundaries | Layered modules before evidence-driven crate extraction | [ADR 0002](../adr/0002-module-and-crate-dependency-direction.md), Accepted |
| AD-002 | Table execution model | One serialized actor per table | [ADR 0001](../adr/0001-server-authority-and-table-actor.md), Accepted |
| AD-003 | Network transport | Bounded loopback TCP with length-prefixed JSON for local alpha; WebSocket/TLS reconsidered before remote exposure | [ADR 0011](../adr/0011-bounded-loopback-tcp-json-framing.md), Accepted |
| AD-004 | Persistence and replay | Versioned between-hand registry checkpoint; active-hand replay excluded from this release slice | [ADR 0014](../adr/0014-between-hand-checkpoint-and-restart-boundary.md), Accepted |
| AD-005 | Secure random source | Entropy-seeded production source plus explicit deterministic review and trusted training-only deal sources | [ADR 0005](../adr/0005-authoritative-randomness-and-deterministic-review.md), [ADR 0016](../adr/0016-training-only-deal-plans-and-policy-boundary.md), Accepted |

## Decision states

- Proposed: written but not approved
- Accepted: controls implementation
- Superseded: replaced by a linked decision
- Rejected: considered and intentionally not selected

D-043 (Sprint 18): network command identity uses a random per-client prefix plus
sequence, independent of display names; ordinary wire validation and server
idempotency remain unchanged. Same-name/space-containing-name process regression.

## 2026-09-09: listed password games

Accepted user policy: open and password-protected games share the lobby. Host
sets an optional password; Join remembers the server, browses and refreshes.
Legacy hidden games remain hidden. ADR [0020](../adr/0020-listed-password-games.md)
records wire 5 / lobby 2 / checkpoint 4 / profile 2 and verifier compatibility.
No LAN transport or Pi hardware prerequisite is introduced by this sprint.

## 2026-09-09: Linux dedicated host

[ADR 0021](../adr/0021-linux-dedicated-server-deployment.md): supplied Fedora 44
x86_64 hardware replaces the Pi assumption for Sprint 20. Native locked build,
manifested release, user-owned service and separate private state. Loopback plus
SSH forwarding validates remote operation without claiming direct VLAN transport.

## 2026-09-09: automatic private LAN connection

Owner extended Sprint 20 and confirmed network-admin-approved TCP 6969.
[ADR 0022](../adr/0022-automatic-lan-tls.md) supersedes the loopback/SSH player
flow: verified TLS to 192.168.5.250:6969, embedded public CA, private host keys,
bounded admission and automatic Host/Join. SSH is operator-only. Public release
and cross-VLAN rollout proof beyond the tested development PC remain separate.
