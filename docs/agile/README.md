# Agile Delivery Hub

This directory contains the living control artifacts for agent-first delivery.

## Daily entry points

- [Current sprint](CURRENT_SPRINT.md): active goal, committed stories, acceptance, and sprint evidence
- [Canonical delivery loop](ITERATION_LOOP.md): exact trigger, autonomous execution contract, and stop conditions
- [Status](STATUS.md): latest handoff, blockers, and next work
- [Backlog](BACKLOG.md): ordered epics and stories
- [Decisions](DECISIONS.md): accepted and pending decisions
- [Risks](RISKS.md): active delivery and product risks
- [Release plan](RELEASE_PLAN.md): incremental releases and go/no-go gates
- Sprint 12 review (local archive: `rituals/2026-08-31-sprint-12-review.md`): private-beta milestone, accepted/remaining points, evidence, and release boundary
- Player-experience release rebase (local archive: `rituals/2026-09-01-player-experience-release-rebase.md`): 660-point baseline, installed-product release gate, and rebased inactive Sprint 14
- Functional single-table tournament rebase (local archive: `rituals/2026-09-01-single-table-tournament-release-rebase.md`): 55-point registration-to-winner target, 34-point multi-table expansion, and revised Sprints 14-20 sequence
- Player-experience backlog refinement (local archive: `rituals/2026-09-01-player-experience-backlog-refinement.md`): refined 108-point E12 stories and Sprints 14-20 ordering
- Installed-shell tangent retrospective (local archive: `rituals/2026-09-01-installed-shell-tangent-retrospective.md`): lessons and controls from the out-of-sprint design/install/Bash path
- Rebased Sprint 14 recommendation (local archive: `rituals/2026-09-01-sprint-14-player-experience-recommendation.md`): inactive 40-point installed shell and repeatable Quick Practice goal
- Sprint 14 planning (local archive: `rituals/2026-09-01-sprint-14-planning.md`): sequenced commitment, acceptance scenarios, token controls, cross-shell matrix, review trajectory, and activation contract
- Historical remaining milestones plan (local archive: `rituals/2026-08-31-remaining-milestones-sprint-plan.md`): prior 552-point/Sprints 13-17 baseline, superseded prospectively after Sprint 13
- [GitHub setup](GITHUB_SETUP.md): project-board fields, labels, and automation conventions
- [Sprint review PDF standard](SPRINT_REVIEW_REPORT_STANDARD.md): mandatory screenshots, one-hand trajectory, and visual QA gate
- [Agent delivery calibration](DELIVERY_CALIBRATION.md): observed active clock, point calibration, forecasting model, and variance rules
- [Sprint token budget policy](TOKEN_BUDGET_POLICY.md): target, runtime ceiling, checkpoints, telemetry, and three-sprint pilot
- [Templates](templates/): sprint planning, review, and retrospective records
- Ritual records (local archive: `rituals/`): kickoff, decision, refinement, planning, and review outcomes

Supporting product documents:

- [Networked multiplayer requirements](../../NETWORKED_MULTIPLAYER_REQUIREMENTS.md)
- [Sneaky Blinders player-experience requirements](../../SNEAKYBLINDERS_PLAYER_EXPERIENCE_REQUIREMENTS.md)
- [Ratatui and TachyonFX UI map](../development/RATATUI_TACHYONFX_UI_MAP.md)
- Private-beta capacity and recovery targets (local archive: `../operations/PRIVATE_BETA_TARGETS.md`)
- Private-beta playtest/operator quickstart (local archive: `../operations/PRIVATE_BETA_QUICKSTART.md`)
- Agile delivery assessment (local archive: `../../AGILE_DELIVERY_ASSESSMENT.md`)
- Agent operating method (local archive: `../../AGENTS.md`)
- [Architecture decisions](../adr/README.md)
- Parallel policy-learning harness (local archive: `../../agents/README.md`): implemented
  training environment, explicit non-capabilities, and ADR-governed boundaries;
  excluded from the 660-point release roadmap until separately prioritized

## Method

The user is the product owner. Agents take the smallest ready vertical slice, implement it through real boundaries, prove it with tests or other executable evidence, and update status. Two-week sprints provide a human review rhythm, but agents do not wait for sprint boundaries when more work is clearly authorized and required for the requested outcome.

The current sprint goal is stable. Individual stories may be split, reordered, or replaced when evidence shows a better path, provided the goal and scope remain intact.

## Artifact ownership

| Artifact | Updated when |
|---|---|
| `CURRENT_SPRINT.md` | Story state, sprint evidence, or sprint scope changes |
| `STATUS.md` | Every material handoff |
| `BACKLOG.md` | Work is discovered, reordered, split, or completed |
| `DECISIONS.md` | A decision is proposed, accepted, superseded, or rejected |
| `RISKS.md` | A risk or mitigation materially changes |
| `RELEASE_PLAN.md` | Release scope or gate changes |
| `TOKEN_BUDGET_POLICY.md` | The pilot, checkpoint contract, or authorization semantics change |
| `DELIVERY_CALIBRATION.md` | Every sprint closure and every material clock/token calibration change |
| ADR | A material architectural choice is made |
| `docs/agile/reports/sprint-N/` | Every Sprint 2+ review; source, screenshots, hand trajectory, and visual QA |
| `output/pdf/sprint-N-review-report.pdf` | Every Sprint 2+ review after complete visual inspection |

## Status vocabulary

- `Backlog`: understood but not ready or not selected
- `Ready`: acceptance and dependencies are sufficient to start
- `In progress`: actively being implemented
- `Review`: implementation exists and awaits evidence or review
- `Done`: Definition of Done is satisfied
- `Blocked`: cannot progress without a named dependency or decision
- `Dropped`: intentionally removed from scope with rationale

Only one primary story should be in progress per agent.

## Estimation

Use Fibonacci points: 1, 2, 3, 5, 8, and 13. Points express relative complexity and uncertainty, not elapsed agent time. Thirteen-point stories should normally be split before implementation.

Forecasts must be updated from observed throughput and risk. They are planning aids, not promises.

Every sprint recommendation and activated sprint also records a separate active-agent-clock forecast. The current provisional model and its limits are maintained in [Agent Delivery Calibration](DELIVERY_CALIBRATION.md). Clock budgets are control signals and never redefine points or stop an otherwise authorized delivery loop.

The completed Sprints 10-12 pilot established the [Sprint Token Budget Policy](TOKEN_BUDGET_POLICY.md). Aggregate targets, ceilings, and checkpoints remain available for explicitly budgeted sprints; direct point-to-token conversion is not supported. No budget permits reduced acceptance, skipped evidence, or a false Done state.
