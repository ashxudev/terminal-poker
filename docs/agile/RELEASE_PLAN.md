# Release Plan

Current as of 2026-09-09: Sprint 19 completed - Game lobby.
733 forecast / 618 accepted / 115 remaining; 21 lobby points accepted.
See [current sprint](CURRENT_SPRINT.md), [backlog](BACKLOG.md), and
[ADR 0019](../adr/0019-dedicated-server-ownership.md).

Current installed wire v5 / lobby v2; paired client/server installation verified. Unified presentation replaces selectable
themes/motion. Local D1 and concurrent ring-game capabilities are accepted;
protected LAN transport, Pi deployment, active-hand crash recovery, D2 tournament
movement and public release are not yet claimed. Formal source/CI gate remains.

## Historical release and delivery record

The following retains its dated evidence. Older sprint numbers, protocol versions,
forecasts and appearance options do not override the current direction above.

# Release Plan

Last updated: 2026-09-08

Rebase record: Functional Single-Table Tournament Release Rebase (local archive: `rituals/2026-09-01-single-table-tournament-release-rebase.md`)

## Sprint 17 completion

Tournament showdown rules accepted 26 new points and are installed for Practice
and private Host/Join. Current product forecast: 699; accepted: 584; remaining:
115. The 673-point/Sprint-16 figures below are historical. Protocol and wire
version 2 require every peer to restart on the updated build. All 295 applicable
tests and installed cross-shell smokes pass; the nine-page review passed visual QA.
Review PDF (local archive: `../../output/pdf/sprint-17-review-report.pdf`).
Custom Practice is recommended next, inactive. D2 is not activated or renumbered.

## Release evidence rule

Historical sprint acceptance remains valid for the scope demonstrated in each review. A component, reducer, renderer, or in-process runtime may be Done without satisfying a release gate.

A playable release gate requires the normal user-facing executable path. Review fixtures and `review-*` binaries may provide supporting evidence, but they cannot substitute for independently launched server/client processes, normal input handling, documented startup commands, or the complete player journey named by the release.

The player-experience backlog refinement (local archive: `rituals/2026-09-01-player-experience-backlog-refinement.md`) added the original 108-point PX epic before the unchanged 89-point tournament and 34-point final-public allocations. The tournament rebase divides those 89 points into a 55-point functional single-table target and a 34-point post-target multi-table expansion. The accepted post-Sprint-15 responsive-table refinement adds 13 PX points, producing the historical 673-point product baseline. Sprint 16 leaves 558 accepted and 115 remaining. Release gates remain authoritative when point progress and release readiness differ.

Sprint 15 accepted the complete 76-point D1 critical path: 21 private Host/Join
points plus all 55 D1 points. Sprint 16 then accepted the 13-point responsive
portrait table. Eight-point Custom Practice and five-point public Join/reach
remain behind D1 because neither was required for a private-invite human
tournament.

| Sprint | Allocation | Points | Outcome |
|---:|---|---:|---|
| 15 | Private Host/Join + D1 | 76 | Accepted: functional installed single-table tournament from setup to winner |
| 16 | Responsive portrait table | 13 | Accepted: one production renderer and installed visual-evidence matrix |
| Candidate | Remaining PX | 47 | Custom Practice, Study, Range Explorer, packaging, public reach, accessibility, and full PX evidence |
| Candidate | D2 | 34 | Multi-table tournament balancing, movement, breaking, consolidation, and hand-for-hand |
| Candidate | Public hardening | 34 | Technical public candidate; external go-live authority remains separate |
|  | **Remaining** | **115** |  |

Sprint 12 achieved a controlled private ring-game beta on the bounded local transport. This does not authorize internet exposure or close Release C's TLS, deployment, remote-CI, and public operational gates.

Sprint 14 formally accepts the installed Home, approved nine-seat table,
repeatable projection-authorized Quick Practice, table console, local profile,
cross-shell lifecycle, and human replayability gate. It closes PX1, not the full
installed player-experience milestone; controls remain functional but require
discoverability iteration in later PX work.

The policy-learning harness is a parallel research programme, not a release gate
or a hidden addition to the 673-point estimate. Its implemented deterministic
one-hand environment, baseline policies, CLI, and throughput benchmark prove an
execution boundary only; they do not prove strategy quality or authorize a
learned bot. Any checkpoint entering Practice, private Host, or later public play
requires separate evaluation, promotion, privacy, bot-policy, and deployment
decisions under ADR 0016 and D-018.

## Current release baseline

| Release | Honest state | Proven | Missing gate |
|---|---|---|---|
| A - Core engine alpha | Accepted local alpha | Multiway rules, pots, showdown, invariants, one server authority, ADR-governed offline adapter, and conformance | Retain conformance and regression gates during later extraction |
| B - Single-table network alpha | Functionally complete local candidate; formal gate blocked | Protocol, privacy, authority, bounded TCP, normal server/client executables, 2-9 processes, reconnect, responsive renderer | Cohesive checkpoint of the more-than-70-path dirty worktree and remote CI pass, both separately authorized |
| C - Multi-table ring-game beta | Accepted controlled local beta | Public/private/unlisted discovery and admission, bounded waiting/promotion, ring lifecycle and blind debt, structured health, safe histories, 8-table/32-session capacity, reconnect, graceful between-hand restart, and zero known critical/high defects | Formal release chain still inherits Release B source/CI gate; non-loopback exposure requires final public security and operations work |
| PX - Installed player experience beta | 74/121 accepted; milestone open | Installed Home, shared responsive table, repeatable authorized Quick Practice, private Host/Join, console, profile, cross-shell restoration, and positive human replayability verdict | Custom Practice, Study, Range Explorer, packaging, public reach, intuitive controls, accessibility, installed E2E, and full PX review remain |
| D1 - Functional single-table tournament | Accepted local milestone | Installed private Host/Join, bounded setup, registration, one-table assignment, authoritative levels/timer, repeated hands, bust-outs, standings/play-money payouts, recovery, and human replayability | Retain regression gates; does not claim D2 movement or public reach |
| D2 - Multi-table tournament expansion | Planned; 0/34 implemented | D1 plus many-table registry/runtime | Balancing, atomic movement, breaking, route handoff, final-table consolidation, hand-for-hand, and multi-table candidate evidence |
| E - Public release | Planned; 34 final technical points remain | Durable private-beta credentials, protected access, history, drain, soak, local quality/capacity/recovery foundations | Public identity, remote transport/TLS, deployment, failure/security review, disaster recovery, support, remote CI, and go-live authority |

## Release A: Core engine alpha

Outcome: deterministic two-to-nine-seat poker rules run locally and preserve offline bot play.

Status: Accepted local alpha. ADR 0012 makes `MultiwayHand` the sole server rules authority and retains `GameState` only as a heads-up offline compatibility adapter. Cross-path conformance and the complete offline/network regression gates pass.

Gate:

- Turn order and blinds pass at every occupancy.
- Multiway betting and short-all-in rules pass accepted examples.
- Main and side pots resolve correctly.
- Chip conservation and card uniqueness property tests pass.
- Existing offline bot smoke tests pass.

## Release B: Single-table network alpha

Outcome: two to nine remote terminal clients complete hands through one authoritative server table.

Status: Local candidate. The user-launchable bounded loopback process boundary, direct network TUI, 2-9 process matrix, and disconnect/fresh-snapshot recovery pass locally. Formal acceptance still requires separately authorized source-control checkpointing and remote CI.

Gate:

- Every action is server-validated.
- Private-view tests show no hidden-card leakage.
- Duplicate and stale commands cannot mutate state twice.
- A documented production server command and normal terminal-client command run as independent processes over a bounded real transport.
- The live keyboard/input loop submits through the projection client; it does not mutate `GameState` or rely on a review fixture.
- Two human-operable terminal clients complete a continuous hand, while automated process coverage exercises occupancies two through nine.
- Disconnect disables controls and reconnect restores the correct authorized seat and fresh view before play resumes.
- Nine-seat TUI is usable at the declared minimum size.
- Existing offline play remains available through its documented entry path.
- The committed source tree passes remote CI before Release B is accepted; commit, push, and remote workflow actions require separate product-owner authorization.

## Release C: Multi-table ring-game beta

Outcome: players create, discover, join, leave, and reconnect to many independent tables.

Status: Accepted controlled local beta. Normal optimized binaries prove public/private discovery, fail-closed admission, bounded waiting/promotion, repeated isolated hands, scoped rotating credentials, protected access material, durable safe histories, signal drain, a two-hour 8-table/32-session soak, graceful checkpoint/restart, and zero known critical/high defects. Active-hand recovery, public identity, non-loopback transport/TLS, deployment, and public exposure are not claimed. Formal Release B source-control/remote-CI acceptance remains separately authorized work.

Gate:

- Table isolation and routing tests pass.
- Waiting lists and seat reservations are race-safe.
- Between-hand active-table recovery is demonstrated; any stronger active-hand RPO must be explicitly accepted and tested.
- Capacity and mass-reconnect targets pass.
- Local limits, privacy-safe health, diagnostics, and operator quickstart are active.
- No known critical or high-severity defects.

TLS is not a gate for loopback-only controlled beta. It is mandatory under Release E before any non-loopback or public exposure.

## Release PX: Installed player experience beta

Outcome: one installed `sneakyblinders` application carries ordinary Practice,
Host, Join, Study, settings, help, recovery, and quit journeys without player
shell choreography or internal identifiers.

Status: 74/121 accepted; milestone not accepted. The user-level command opens a
production Home, repeatable projection-authorized nine-handed Quick Practice,
and private Host/Join through the accepted responsive portrait table and
persistent table console. The product owner judged the remediated build
functional and would play again; control intuitiveness remains open. Custom
Practice, public Join/reach, Study, Range Explorer, packaging, accessibility,
and full PX acceptance retain 47 points.

Gate:

- CMD, PowerShell, and Git Bash launch the same no-flag Home from arbitrary
  directories and always restore the terminal.
- Quick and Custom Practice support the accepted two-to-nine-seat scope and a
  repeatable results-to-next-hand lifecycle through authorized projections.
- This-computer Host and Join supervise and consume the accepted authoritative
  registry, credentials, reconnect, history, checkpoint, and drain boundaries.
- Study exposes authorized histories, replay, statistics, notes, Learn, and a
  provenance-labelled Range Explorer without claiming unapproved strategy.
- Profiles, settings, accessibility, reduced motion, packaging, upgrade,
  uninstall, diagnostics, and data preservation pass migration/failure tests.
- Installed end-to-end journeys, first-time usability, one continuous hand, PDF,
  and complete visual inspection pass with zero known critical/high defects.
- New journeys expose self-describing controls and are qualitatively tested for
  discoverability, not only keyboard correctness.

## Release D1: Functional single-table tournament

Outcome: two through nine human players complete one networked, play-money,
single-table freezeout from installed Host/Join registration to one winner and
deterministic standings.

Status: Accepted local milestone. Sprint 15 combined 21 tournament-critical
private Host/Join points with all 55 entry/start/completion/recovery points and
passed the optimized gate, installed journey, deterministic evidence, PDF, and
positive human review. D1 does not claim Custom Practice, public Join/reach, the
complete PX milestone, or multi-table tournament capability.

Gate:

- The ordinary `sneakyblinders` journey creates or joins a fixed-start
  tournament, registers 2-9 entrants, and assigns every entrant once.
- Before registration locks, the host can configure bounded table capacity,
  starting stack, starting blinds/antes, level durations and schedule, breaks,
  and a play-money payout/result structure, with a complete preview and explicit
  validation. After start, the server owns the immutable configuration and
  authoritative scheduled progression.
- Every entrant receives one seat and complete starting stack at one table.
- One authoritative monotonic schedule applies levels, antes, and breaks exactly
  once at documented between-hand boundaries.
- Disconnect, timeout, and reconnect retain the correct identity, seat, stack,
  schedule, and authorized projection.
- Eliminations occur exactly once; placement, remaining-player count, winner,
  standings, configured play-money payouts, chips, and cards reconcile
  deterministically.
- Controlled between-hand restart preserves tournament and table state without
  duplicate entry, award, elimination, or winner.
- Optimized normal-process and installed TUI evidence completes registration to
  winner with zero known critical/high defects; screenshots, one-hand trajectory,
  visual QA, and PDF review pass.

Explicitly not in D1: balancing, cross-table movement, table breaking,
final-table consolidation, hand-for-hand coordination, public internet exposure,
late registration, re-entry/rebuy/add-ons/bounties, real-money or transferable
prizes, arbitrary host mutation of a live table, or learned-policy deliverables.

## Release D2: Multi-table tournament expansion

Outcome: one tournament safely spans multiple table authorities and consolidates
to a final table and winner.

Status: Planned post-target, inactive and unnumbered. D2 retains 34 points from the original
tournament allocation; D1 acceptance is a prerequisite.

Gate:

- Deterministic balancing selects a valid source, player, destination, and blind
  treatment without oscillation or starvation.
- Atomic move and route handoff cannot duplicate, lose, or double-deal a player
  or stack across retries or recovery.
- Tables break only after all survivors have valid assignments.
- Final-table consolidation preserves identity, seats, stacks, button, and blind
  state.
- Configured hand-for-hand play coordinates without leaking early outcomes.
- Normal-process evidence crosses levels, balances, moves, breaks, consolidates,
  reconnects, and continues without cross-table leakage.

## Release E: Public release

Outcome: evidence-backed public play-money service.

Status: Planned, not active. Sprint 13 accepted the first 39 public-hardening points; rebased Sprint 18 retains the final 34, but identity, deployment ownership, public capacity, recovery objectives, remote transport, external infrastructure, and go-live authority remain decision gates. D1 is the nearer functional target; neither D1 nor D2 self-authorizes public release.

Gate:

- Security and privacy review complete.
- Soak and failure-injection tests pass.
- Backup and recovery exercise passes.
- Upgrade and rollback are demonstrated.
- Support and incident ownership are agreed.
- Product, engineering, QA, and operations explicitly approve go-live.

## Non-goals

- Real-money gaming
- Payments or withdrawals
- Regulatory compliance programme
- Native mobile or graphical clients
- Multi-region active-active operation at initial launch
- Learned-policy research or model training as a prerequisite for the installed
  player-experience, tournament, or public-release milestones
