# Canonical Delivery Iteration Loop

## Invocation

The exact canonical trigger is:

```text
RUN DELIVERY LOOP
```

The phrase is intentionally distinctive and repository-specific. It means: execute one complete active-sprint delivery loop, not merely report status or perform agile ceremonies.

## Trigger contract

On invocation, the delivery agent must:

1. Synchronize with repository and delivery state.
2. Select the next work needed to achieve the active sprint goal.
3. Refine uncertainty only as much as required to execute safely.
4. Implement the work through real product boundaries.
5. Validate it with evidence proportional to risk.
6. Review and accept or reject the result against the story criteria.
7. Update the living artifacts.
8. Repeat the story cycle while the sprint goal remains incomplete and ready work exists.
9. Conduct sprint review and retrospective only when sprint evidence exists.
10. Stop after closing the active sprint or reaching a genuine stop condition.

For a sprint with an explicitly accepted token budget, the loop also creates one sprint-scoped budgeted runtime goal, samples it at the checkpoints defined in `TOKEN_BUDGET_POLICY.md`, and stops before exceeding its authorized ceiling if the product owner has not approved more capacity. A planning target is not a stop condition.

The loop is sprint-scoped. It does not automatically activate the next sprint.

## Release rebase ritual

The exact release-reassessment trigger is:

```text
RUN RELEASE REBASE
```

Invoke it only when no sprint is active or when the active sprint has been explicitly stopped. It performs planning and evidence reconciliation rather than product implementation:

1. Reconstruct the accepted product vision and staged release outcomes.
2. Inspect normal executables, integration boundaries, source-control state, and release evidence.
3. Preserve historical sprint/story acceptance while distinguishing component completion from release completion.
4. Reassess release gates, backlog estimates and ordering, decisions, and risks.
5. Refine the next recommended sprint around the smallest complete release-risk boundary.
6. Record a ritual outcome and leave the recommendation inactive.

The trigger authorizes local repository documentation and planning updates. It does not authorize feature code, sprint activation, commits, pushes, remote workflows, deployment, publication, or product-policy invention. A subsequent explicit activation is required before `RUN DELIVERY LOOP` can execute the recommendation.

## Authority granted by the trigger

The trigger authorizes:

- Reading the repository and relevant local documentation
- Creating and editing files inside the repository
- Refactoring code required by the active sprint goal
- Adding and updating tests
- Running local build, test, lint, formatting, and diagnostic commands
- Updating backlog, sprint, status, decision, risk, ADR, and ritual records
- Making reversible implementation decisions inside accepted architecture and product policy
- Continuing through multiple ready stories without another prompt

The trigger does not authorize:

- Git commits, pushes, branches on a remote, pull requests, or releases
- Deployment or publication
- Changes to remote GitHub projects, labels, issues, or repository settings
- Accessing or creating secrets and credentials
- Destructive file, data, infrastructure, or account actions
- System-wide software installation
- Spending money or provisioning paid services
- Real-money gaming work
- Product-scope expansion beyond the active sprint goal
- Resolving material player-experience or poker-policy choices on behalf of the product owner

Normal higher-priority safety, tool, and user instructions continue to apply.

## Full loop

```text
Trigger
  -> Synchronize
  -> Check sprint goal and authority
  -> Select/refine next ready story
  -> Plan the smallest complete slice
  -> Implement
  -> Validate
  -> Review against acceptance and Definition of Done
  -> Update artifacts
  -> Goal met?
       No  -> select next ready story and repeat
       Yes -> capture review evidence -> build and inspect PDF -> sprint review -> retrospective -> close sprint -> handoff
```

## Phase 0: Trigger acknowledgement

Confirm concisely:

- The sprint goal being executed
- The initial active story
- Known blockers or unverified conditions
- Any action that remains outside the trigger's authority
- The accepted token target, runtime ceiling, and first checkpoint when the sprint is budgeted

Begin work immediately after acknowledgement. Do not ask for confirmation when existing sources make the next action clear.

## Phase 1: Synchronize

Read and reconcile:

- `AGENTS.md`
- `docs/agile/CURRENT_SPRINT.md`
- `docs/agile/STATUS.md`
- Relevant backlog, decisions, risks, requirements, and ADRs
- Current `git status`
- Affected code and tests

Preserve unrelated user changes. If artifact states disagree, correct them using the highest applicable source before selecting work.

Output of this phase:

- One current sprint goal
- One active story or next ready story
- Named blockers and dependencies
- A short execution plan

## Phase 2: Goal and authority check

Before implementation, confirm:

- The active sprint goal is not already achieved.
- The selected work is necessary for that goal.
- The work is within the trigger's local repository authority.
- No unresolved decision materially changes the implementation.

If one story is blocked, select the next independent ready story required by the goal. Stop only when every meaningful route toward the goal shares the same genuine blocker.

## Phase 3: Select and refine

Choose work in repository priority order:

1. Critical correctness, privacy, security, or recovery issue
2. Sprint-goal blocker that can be resolved locally
3. Smallest ready critical-path story
4. Evidence or documentation required to finish an implemented slice

Refinement should produce:

- Observable acceptance criteria
- Explicit exclusions
- Identified authority and privacy impact
- Test or evidence plan
- Dependencies and decisions
- A point estimate for relative complexity and a separate active-clock forecast from `DELIVERY_CALIBRATION.md`
- A separate token planning target, runtime ceiling, and checkpoints when the recommendation is budgeted

Split work that cannot be completed and validated as one coherent slice. Use a time-boxed spike only when direct implementation cannot safely reduce the uncertainty.

## Phase 4: Plan

Maintain a short live plan for non-trivial work. At most one item is actively being implemented by an agent at a time.

Record the trigger timestamp and forecast range at activation. For a budgeted sprint, create one runtime goal with the accepted ceiling, confirm the live meter, and record its initial reading. A clock checkpoint or token-target crossing records variance and updates the forecast; neither weakens the sprint goal or Definition of Done.

Follow `TOKEN_BUDGET_POLICY.md` for token attribution and sampling. Do not reset or replace the runtime goal to create apparent capacity. If the meter is unavailable, do not simulate consumption; request direction before feature implementation.

Plan vertical behaviour instead of file layers where practical. A useful slice may cross:

- Rules model
- Protocol contract
- Server authority
- Client projection
- Tests and observability

Do not start unrelated cleanup.

## Phase 5: Implement

For each story:

1. Establish a failing test, invariant, or reproducible baseline when practical.
2. Implement the smallest domain-correct change.
3. Exercise the real boundary carrying the main risk.
4. Preserve offline behaviour unless the story explicitly migrates it.
5. Add error and rejection behaviour.
6. Keep the diff cohesive and inspectable.

Poker, authority, privacy, protocol, and recovery guardrails in `AGENTS.md` are mandatory throughout.

## Phase 6: Validate

Run validation from narrow to broad:

1. Focused unit or scenario tests
2. Relevant property and invariant tests
3. Component or multi-client integration tests
4. Formatting and linting
5. Full applicable test suite
6. Manual or visual verification where automated evidence is insufficient
7. Recovery, load, or failure tests when the story changes those risks

Record exact results. Never claim a check passed if the tool did not run.

When a check fails:

- Diagnose and fix in scope.
- Add a regression test for reproducible defects.
- Re-run the failed check, then the broader gate.
- Mark the story Blocked only after safe alternatives are exhausted and the blocker is named.

## Phase 7: Review and acceptance

Perform a self-review of the full diff and evidence:

- Are acceptance criteria met?
- Are server authority and private-state boundaries preserved?
- Can invalid or duplicate input mutate state incorrectly?
- Are poker invariants covered?
- Did unrelated files change?
- Are documentation and operational effects current?
- Is there an untested migration or rollback risk?

Story result:

- `Done` only if Definition of Done is satisfied
- `Review` if implementation exists but required evidence is incomplete
- `Blocked` if a named external or product dependency prevents completion
- `In progress` if more authorized implementation remains

## Phase 8: Synchronize artifacts

After each material story result:

- Update `CURRENT_SPRINT.md` state and evidence.
- Update `STATUS.md` outcome, active work, blocker, and next action.
- Update `BACKLOG.md` ordering and discovered work.
- Update decisions and ADRs when choices were made.
- Update risks when exposure or mitigation changed.
- Record the cumulative token meter and expected final consumption immediately when each story is accepted in a budgeted sprint, before marking or starting the next story. Convergent implementation does not permit one combined reading to replace separate acceptance checkpoints.
- Update requirements or release gates only when scope or evidence changed.

Artifacts describe actual evidence, not optimistic intent.

## Phase 9: Loop decision

### Continue

Continue automatically when:

- The sprint goal is incomplete.
- Another necessary story is Ready.
- A blocker affects only one route and independent useful work remains.
- A failed check can be diagnosed or fixed safely in scope.
- An explicitly accepted runtime ceiling still has enough remaining capacity for the next coherent slice and its required validation.

### Enter sprint closure

Enter closure when all sprint-goal acceptance conditions have evidence and every committed story is Done, Dropped with rationale, or moved with an explicit goal-preserving decision.

### Stop blocked

Stop only when all meaningful progress toward the sprint goal requires one of:

- A material product-owner choice
- New external authority
- A secret, account, service, or infrastructure change not authorized by the trigger
- A system-wide tool installation
- Resolution of conflicting poker rules affecting chips or action rights
- Recovery from an unsafe or destructive condition
- User changes that cannot be preserved while implementing the goal
- An explicitly accepted sprint token ceiling would be exceeded without additional product-owner authorization

The blocker report must name the decision or action required and identify what was completed before stopping.

## Phase 10: Sprint closure

Closure is evidence-driven and occurs in this order:

1. Run the sprint's complete applicable quality gate.
2. Capture at least two screenshots of key executable updates.
3. Capture one complete hand as a continuous trajectory from initial state to terminal outcome.
4. Reconcile the hand's actions, board, pot, contributions, and stacks in `hand-trajectory.md`.
5. Generate the human-readable PDF report required by `SPRINT_REVIEW_REPORT_STANDARD.md`.
6. Render every PDF page to PNG and visually inspect the complete final render.
7. Fix defects and repeat full rendering and inspection until every page passes.
8. Record page results and the final PDF SHA-256 in `visual-qa.md`.
9. Conduct sprint review against the sprint goal, not story count, using the inspected PDF.
10. Record accepted demonstrations and incomplete outcomes.
11. Review release-gate impact.
12. Conduct a concise retrospective based on observed delivery evidence.
13. Select no more than two owned improvement actions.
14. Mark the sprint Done or Partially achieved.
15. Update status and backlog.
16. Identify the recommended next sprint goal without activating it.
17. Record active delivery blocks, excluded inter-sprint/user gaps, corrective rework, total active clock, seconds per accepted point, and forecast variance.
18. For a budgeted sprint, record every required checkpoint, final tracked tokens, tokens per accepted point, tokens per active minute, target and ceiling variance, and meter anomalies in the sprint record and `DELIVERY_CALIBRATION.md`.

Do not fabricate a review or retrospective when no implementation evidence exists.

From Sprint 2 onward, do not mark a sprint Done without the final PDF, retained source screenshots, one-hand continuity ledger, and SHA-bound visual QA record. If capture, PDF generation, rendering, or visual inspection is unavailable, retain the sprint in Review or mark it Blocked with the exact missing capability.

## Communication cadence

During a running loop:

- Send a brief kickoff update before tool use.
- Send an update at meaningful phase or story transitions.
- Do not leave the user without an update for more than 60 seconds during ongoing work.
- Surface assumptions and blockers as soon as they matter.
- Keep intermediate updates concise; the final handoff is self-contained.

## Completion handoff

The final response must state:

1. Sprint goal result
2. Stories completed, moved, or blocked
3. Important files changed
4. Tests and checks with exact results
5. Decisions and assumptions
6. Residual risks or unverified areas
7. Recommended next sprint goal
8. Link to the final visually inspected PDF sprint review report
9. For a budgeted sprint, planning target, runtime ceiling, final token consumption, material variance, and meter reliability

## Examples

### Canonical invocation

```text
RUN DELIVERY LOOP
```

This runs the active sprint to completion or a genuine stop condition.

### Not equivalent

```text
Give me a status update.
```

This requests reporting only and does not authorize implementation.

```text
Work on E2.3.
```

This authorizes the named story but does not invoke automatic sprint closure or continuation through the remaining sprint.

```text
RUN DELIVERY LOOP and deploy it.
```

The delivery loop runs locally. Deployment remains a separate externally visible action and requires explicit valid deployment authority and release-gate evidence.
