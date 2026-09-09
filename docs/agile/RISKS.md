# Current risk update - 2026-09-09

Sprint 18 risk: client exit must not own server shutdown; test a separately
running process with multiple games after creator disconnect. Transport stays
loopback-only; do not claim Pi/VLAN/security readiness. R-015 source checkpoint
remains open. R-016's Sprint 15 budget is historical, not an active constraint.
D1 setup/clock risks now retain regression coverage from accepted Sprint 15.
The current UI has no high-contrast/motion choices. Older rows below retain
historical context; current scope is in CURRENT_SPRINT.md and ADR 0019.

# Risk Register

Last reviewed: 2026-09-08

Branded Home relies on terminal graphics detection at >=100x36. Unsupported,
compact, high-contrast and NO_COLOR sessions retain working text navigation.
Actual installed bitmap screenshot inspected; live image cleanup after route
changes/resize awaits human review. Shared menu routing and layout/cache tests
pass. Delivery evidence (local archive: `rituals/2026-09-08-branded-menu.md`).



Current correction: automatic mucking removes the human showdown decision window.
Clockwise checked-river order replaces best-first. Protocol/wire v4 rejects v3
peers because optional-decision fields were removed; restart all peers together.
Public histories still exclude private mucks. No indefinite hang was reproduced
in the prior two-player TCP test; the removed five-second window explains a
bounded pause, not every possible stall. Full verification is recorded in the
delivery record (local archive: `rituals/2026-09-08-pokerstars-showdown.md`).

Historical prior correction follows.

Post-review correction: reveal-window deadlines are authority-owned and cannot
be extended by duplicate show requests. The checked-river best-first rule is an
explicit product policy, distinct from TDA clockwise procedure. Runtime and
privacy tests cover the five-second choice window; upgrade now requires v3 peers.
Uncalled-shove coverage spans every street and two through nine seats. A remaining
contested main pot must still run out even when a side-pot bet is uncalled.

Historical Sprint 17 evidence follows.

Sprint 17 mitigation: premature/over-broad showdown exposure is replaced by an
authoritative reveal set, private mucks, and pre-runout all-in snapshots. Ordered
privacy, negative intent, reconnect, disconnected runout, and exact-once network
completion are under the sprint gate. Upgrade risk: protocol/wire v2 requires
all peers to restart on the new build. Active-hand disk recovery and live-floor
discretion remain excluded; between-hand recovery is retained.

Score uses probability times impact on a 1-5 scale. Review high risks at every sprint review.

| ID | Risk | Probability | Impact | Score | Trigger | Mitigation | Owner | State |
|---|---|---:|---:|---:|---|---|---|---|
| R-001 | Multiway betting edge cases are underestimated | 1 | 5 | 5 | Randomized sequences reveal a contradiction or conservation defect | E2.1-E2.5 focused scenarios plus the 192-case replayable E2.6 campaign pass across occupancies 2-9 | Tech lead | Mitigated |
| R-002 | Hidden cards leak through views, logs, or errors | 1 | 5 | 5 | Full state crosses protocol boundary | Role-derived player/spectator subscriptions, public errors/events, reveal rules, and negative serialization/capture tests pass; retain through client and transport work | Server owner | Monitoring |
| R-003 | Reconnect and restart recovery diverge | 2 | 5 | 10 | Restored roster/stack/identity or fresh revision differs from the last safe boundary | Atomic validated between-hand checkpoint, monotonic fresh authority, zero-award restore, and forced two-table process restart pass; active-hand recovery remains explicitly open | Platform owner | Monitoring |
| R-004 | Tournament moves duplicate or lose a player | 2 | 5 | 10 | Cross-table move is non-atomic | Cross-table movement is excluded from the Sprint 15 target and isolated in the 34-point Sprint 17 expansion; require one-player/one-seat invariants, deterministic selection, atomic move/route handoff, table-breaking, and recovery tests before any multi-table claim | Tournament owner | Open |
| R-005 | Nine-seat terminal layout is unreadable | 1 | 3 | 3 | UI regresses below the tested 120 x 40 viewport | Responsive 2-9 geometry tests plus inspected 120 x 40 and 160 x 50 actual-Ratatui evidence | Client owner | Mitigated |
| R-006 | Scope expands into identity, chat, spectators, money, or unrefined player surfaces | 3 | 4 | 12 | New scope enters an active sprint or tangent without an explicit estimate and displaced-roadmap tradeoff | The 660-point rebase now includes E12; accounts, public bots/chat/spectators/money, multi-tabling, solver content, and broader hosting still require reforecast before activation | Product owner | Open |
| R-007 | Lobby or reconnect storm overloads a server | 1 | 4 | 4 | Mailbox saturation or unbounded broadcast | All relevant collections are bounded; the optimized 8-table/32-session/32-reconnect profile and waiting/promotion candidate pass. Retain multi-hour soak before broader exposure | Platform owner | Monitoring |
| R-008 | Offline game regresses during engine extraction | 1 | 3 | 3 | Bot mode stops passing smoke test | Seat-indexed migration, shared library, offline suite, release build, and CLI smoke tests are green | Core owner | Mitigated |
| R-009 | Toolchain is not reproducible locally | 1 | 3 | 3 | Project-local gate runner fails in a fresh shell | `scripts/run_rust_gate.ps1` pins the proven stable GNU host and runs the full gate plus release smokes | Delivery lead | Mitigated |
| R-010 | One person becomes sole rules expert | 3 | 4 | 12 | Reviews bottleneck on one owner | Executable examples and rotating review | Delivery lead | Open |
| R-011 | A session can act for or observe the wrong seat or table | 1 | 5 | 5 | Caller-selected audience, seat, or cross-table identity reaches remote ingress | Opaque one-role bindings derive table, hand, seat, and audience; wrong-table process probe and negative authorization/private-stream tests pass | Server owner | Monitoring |
| R-012 | Client state diverges by applying intentions before authoritative output | 1 | 4 | 4 | TUI renders optimistic poker state or misses a revision/stream update | Projection-only reducer, pending lockout, stale/duplicate/gap tests, and fresh-snapshot recovery pass; retain across transport work | Client owner | Monitoring |
| R-013 | Byte transport framing or disconnect handling corrupts protocol order | 1 | 5 | 5 | Partial, coalesced, oversized, or truncated frames reach authority or reconnect replays stale intent | Bounded framing, hostile process ingress, 2-9 independent hands, and fresh-snapshot reconnect pass; retain regression coverage before remote transport selection | Platform owner | Monitoring |
| R-014 | Legacy `GameState` and `MultiwayHand` diverge into two supported poker engines | 1 | 4 | 4 | A server rule or protocol path begins depending on the offline adapter, or heads-up conformance fails | ADR 0012 names one server authority, freezes the narrow offline adapter, and retains executable cross-path conformance plus offline/network regression gates | Core owner | Monitoring |
| R-015 | The multiplayer programme is lost, unreviewable, or never remotely validated because it remains one dirty local worktree | 5 | 5 | 25 | Workspace loss, merge conflict, or local-only assumptions invalidate 469 accepted points | The player-experience rebase found 67 changed/untracked paths on unchanged upstream `main`; obtain explicit product-owner authorization for a cohesive checkpoint and remote CI now, and never silently commit or push | Delivery lead | Open |
| R-016 | A token ceiling causes rushed, incomplete, or falsely accepted sprint work | 4 | 5 | 20 | The 76-point Sprint 15 approaches 650,000 while implementation/full-gate work remains, or 700,000 before the final inspected review is closure-ready | Use one 750,000-token goal; sample every story and phase; finish implementation/focused process evidence near 550,000 and the full gate near 650,000; eliminate unrelated output/tangents; stop and request reauthorization before the ceiling rather than waive acceptance or split the goal | Delivery lead | Active |
| R-017 | Tournament clock, hand boundaries, and recovery diverge | 3 | 5 | 15 | A level/break applies twice or mid-hand, or restart duplicates an entry, elimination, award, or winner | Sprint 15 requires one monotonic schedule, deterministic between-hand application, versioned validated controller/table checkpoints, and normal-process restart-to-winner evidence before D1 closes | Tournament owner | Open |
| R-018 | Public-release gates cannot be exercised without identity, hosting, secrets, or an operational owner | 4 | 5 | 20 | Sprint 18 reaches external integration with decisions or authority missing | Decide PD-003/PD-010 before Sprint 18; separately authorize source control, remote CI, infrastructure, certificates, secrets, deployment, security review, support, and go-live | Product owner | Open |
| R-019 | Caller-selected labels or recoverable checkpoint material become authorization secrets outside the local beta | 1 | 5 | 5 | A session label, join code, or checkpoint copy grants or reveals a route after exposure broadens | Random principals, verifier-only checkpoints, rotating credentials, and negative process/serialization tests pass; retain before remote exposure | Security owner | Monitoring |
| R-020 | Short capacity tests hide memory, latency, reconnect, or recovery degradation over time | 1 | 4 | 4 | Counts/RSS grow after warm-up, latency bounds drift, or a reconnect wave stalls | Formal 900s warm-up plus 7200s at 8 tables/32 sessions passed all intervals with 0.70 MiB RSS growth and zero alerts | Platform owner | Mitigated |
| R-021 | Supported terminal shells interpret input, color, sizing, or restoration differently | 1 | 3 | 3 | An installed command skips Home, loses hierarchy, clips at the declared viewport, or leaves a damaged terminal in CMD, PowerShell, or Git Bash | Installed full journeys pass in CMD, PowerShell, and real Git Bash; standard/compact/minimum, true/restricted color, release filtering, failure, interrupt, and restoration evidence pass. Retain the matrix for later shell integrations | Client owner | Mitigated |
| R-022 | Out-of-sprint UX tangents obscure scope, acceptance, and delivery calibration | 1 | 3 | 3 | Working UI accumulates without a sprint goal, point state, token meter, or review boundary | Refinement/rebase preceded activation; Sprint 14 formally absorbed and revalidated the tangent under 40 points, an explicit meter, full gate, installed evidence, human acceptance, and PDF review | Delivery lead | Mitigated |
| R-023 | Training-only hidden state leaks into policy or production paths, or environment throughput is overstated as learned performance | 1 | 5 | 5 | A policy can observe deal plans/future cards, production accepts a deterministic deal source, private trajectories enter safe histories, or benchmark rates are cited as strategy quality | Enforce ADR 0016 with negative tests, independent randomness, ordinary authority mapping, separated trajectory/history stores, explicit benchmark labels, no human ingestion by default, and a separate promotion/deployment decision | AI/security owner | Monitoring |
| R-024 | A functional single-table tournament is misrepresented as multi-table tournament capability | 2 | 4 | 8 | Review, UI, documentation, or release language implies balancing, movement, breaking, consolidation, or hand-for-hand has shipped | Name the Sprint 15 gate "functional single-table tournament", retain explicit excluded-capability assertions, and require separate Sprint 17 normal-process evidence before any multi-table claim | Product/tournament owner | Open |
| R-025 | Tournament setup control is mistaken for authority to mutate a live table, or invalid stack/blind/timer/payout combinations start | 3 | 4 | 12 | Host UI or protocol permits post-lock edits, non-reconciling payouts, invalid levels, or direct stack/pot/action mutation | Version and bound the draft; provide validation and preview; lock one immutable configuration at start; permit only controller-owned scheduled progression; reject mutation without revision; test invalid combinations and payout reconciliation | Tournament owner | Open |
| R-026 | Responsive table work preserves two hidden designs or makes the minimum view technically render but functionally unreadable | 1 | 4 | 4 | A width/height branch selects different component trees, primary landmarks move across resize, portrait geometry becomes landscape on large terminals, or nine seats lose actionable information at the claimed floor | Sprint 16 replaced the breakpoint swap with one renderer/component order; normalized portrait anchors and 2/6/9-seat tests pass across the 80x30, 72x32, 64x36, and 56x40 support staircase; installed cross-shell and visually inspected Ratatui/PDF evidence pass. Retain this matrix for future table changes | Client owner | Mitigated |

## Kicker correctness follow-up - 2026-09-07

DISC-020 exposed a High-severity omission in complete-hand ranking: secondary
kickers were absent for pairs, two pair, trips, and quads. This could affect
both displayed winning cards and pot settlement. The source repair restores
complete comparison vectors, with played-hand, final-kicker, genuine-tie,
2-9-seat main/side-pot conservation, and production-renderer regressions.
Final-source formatting, strict Clippy, library tests, and the applicable
network/tournament suites pass. Retain these regressions under R-001;
historical hand outcomes are not retrospectively validated by this fix.
Candidate installation is tracked in the
defect record (local archive: `rituals/2026-09-07-kicker-comparison-fix.md`).

## Risk response

- 15-25: active mitigation in the current or next sprint
- 8-14: monitor and schedule mitigation before exposure grows
- 1-7: accept or monitor

New critical privacy, chip-conservation, authorization, or recovery risks interrupt normal feature priority.

## Sprint 19 lobby risks

- Registration departure: cancellation and socket departure release only waiting
  entrants. A concurrent start rejects withdrawal and preserves the live hand.
- Saved privacy: new listed PasswordProtected visibility is separate from legacy
  Private/Unlisted. Version-3 saved hidden tables remain hidden on migration.
- Password transport remains loopback-only. Argon2id verifier storage is not TLS;
  protected LAN transport and admission rate limiting must precede VLAN exposure.
- Remembered IP is server configuration, not automatic VLAN discovery. Pi/DNS
  provisioning is later work; no Pi is needed for the current local journey.

## Sprint 20 Linux host

- Fedora x86_64 is the tested native target; glibc-linked artifacts do not imply
  older-distribution or ARM compatibility. Bundle records toolchain and libraries.
- User service starts without client ownership. Reboot-without-login requires
  lingering; actual machine reboot testing is not inferred on a shared host.
- Existing transport stays loopback. SSH forwarding secures operator validation;
  game passwords do not provide transport encryption or future player identity.
- Observed blocker: Fedora 44 policy 44.8 denies SSH-session TCP forwarding.
  Owner approved scoped policy module and TCP 7777 mapping; installed and
  Windows gameplay verified. Unrelated port 22 remains denied. SELinux enforcing.
- Upgrades stop at a maintenance boundary and preserve state outside releases.
  Existing active-hand crash durability limitation still applies to restarts.

## Sprint 20 direct LAN extension

The initial loopback-only transport risk above is superseded by verified Rustls
TLS and bounded per-IP admission on 6969. Certificate renewal is required before
2028-12-12; CA replacement requires updated clients. See the Linux runbook.
Development-PC reach is verified; every VLAN, shared-NAT budgets, internet
identity/distribution, reboot, ARM compatibility and active-hand crash recovery
are not claimed. The legacy operator SSH policy remains installed but unused by
players. Application-level directory requests retain their existing bounded
timeouts; initial connection cancellation is responsive.
