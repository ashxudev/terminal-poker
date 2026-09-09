# Sprint Token Budget Policy

Status: Pilot completed; aggregate goal budgeting retained

Effective: 2026-08-31, beginning with Sprint 10

Reviewed: Sprint 12 closure

## Purpose

Token budgets bound agent consumption without turning cost control into an excuse for incomplete or unverified delivery. They are separate from story points, active-agent-clock forecasts, model context windows, API rate limits, and release acceptance.

The pilot measures whether goal-level token telemetry is stable enough to forecast future sprints. Sprint 9 is the only retained baseline: 366,526 tracked tokens for 29 accepted points over 21:49 of goal-tracker time.

## Two-tier control

Every activated pilot sprint has both:

- A **planning target**: the expected consumption used for checkpoints and variance analysis. Crossing it triggers reassessment but does not stop authorized work.
- A **runtime ceiling**: the maximum explicitly authorized goal budget. It is a safety boundary, not a completion criterion.

The target and ceiling must include implementation, tests, corrective work, executable screenshots, PDF production, complete visual inspection, retrospective, artifact synchronization, and final handoff.

No story, test, privacy check, review artifact, or Definition-of-Done requirement may be waived to fit either number. A sprint is never Done merely because its budget is exhausted.

## Activation contract

The sprint recommendation and activated sprint record both state the planning target, runtime ceiling, and checkpoint thresholds. When the product owner explicitly activates that recommendation, the delivery agent creates the sprint-scoped runtime goal with the recorded ceiling as its token budget.

If the runtime cannot create or read the budgeted goal, do not invent an estimate or silently run unmetered. Record the missing capability and request product-owner direction before feature implementation.

Use one goal for the complete sprint, from activation through final inspected review and closure. Do not replace, reset, or split the goal to conceal consumption. A materially restarted sprint must retain and report consumption from every associated goal.

## Checkpoints

Read and persist the cumulative meter at least:

1. At activation
2. Immediately when each story is accepted, before the next story moves to Done; a coherent-slice reading may supplement but does not replace individual story readings
3. Immediately before and after the full quality gate
4. Immediately before review-evidence production
5. After final PDF generation and complete visual inspection
6. At sprint closure

At each named threshold:

- Compare actual consumption with completed scope and remaining risk.
- Record the cause of material variance.
- Update the expected final consumption.
- Reduce avoidable context and tool-output volume where safe.
- Preserve the sprint goal and all quality gates.

Routine readings are delivery telemetry and may be recorded without interrupting the product owner. Review visibility, target adherence, ceiling headroom, phase deltas, and meter reliability in the post-sprint review and retrospective. Surface an immediate warning only when the meter becomes unavailable or unreliable, forecast consumption threatens the ceiling, or the final planned checkpoint is reached with acceptance work incomplete.

At the final checkpoint below the ceiling, prioritize leaving a buildable, tested, accurately documented state. If completing the sprint would exceed the authorized ceiling, stop before the ceiling, report the incomplete acceptance boundary, and request an explicit budget change. Keep the sprint in `In progress`, `Review`, or `Blocked` as the evidence warrants; never close it as Done.

## Telemetry record

Every sprint recommendation, sprint record, review, and calibration entry records:

- Model and reasoning configuration when observable
- Planning target and runtime ceiling
- Cumulative tokens at each checkpoint
- Tokens used by implementation, full-gate, and review/evidence phases where checkpoint deltas permit
- Final tracked tokens
- Tokens per accepted point and tokens per active minute
- Target and ceiling variance
- Compaction, corrective rework, unusually large tool output, interrupted execution, or runtime-meter anomalies

The aggregate runtime number is an internal delivery-consumption measure. It is not a billing estimate because it does not provide a stable input, cached-input, reasoning, output, and tool-cost breakdown.

## Calibration and comparability

- Treat Sprint 9 as an observational baseline, not a predictive model.
- Pilot the policy for Sprints 10-12.
- Do not claim a points-to-token forecast until three comparable budgeted sprints have closed.
- Compare sprints only when the model, reasoning configuration, goal boundary, and evidence standard are materially comparable; otherwise segment the sample.
- Recalibrate after the pilot or immediately after a material configuration or process change.
- Keep point estimates and active-clock forecasts independent from token budgets.

## Sprint 10 pilot

- Planning target: 450,000 tokens
- Runtime ceiling: 725,000 tokens
- Checkpoints: 300,000, 450,000, and 600,000 cumulative tokens
- Basis: Sprint 9 used 366,526 tokens; 725,000 approximately covers the upper end of Sprint 10's existing 28-42 active-minute control range at Sprint 9's observed token rate

The Sprint 10 ceiling is intentionally conservative because one observation cannot establish reliable forecast variance.

## Sprint 10 result and Sprint 11 recommendation

Sprint 10 reached its closure checkpoint at 457,988 tokens: 7,988 (1.8%) above its 450,000 planning target with 267,012 (36.8%) of its 725,000 ceiling unused. All acceptance, full-gate, optimized-process, Ratatui, PDF, page-inspection, standards correction, and closure work remained inside the one goal. Crossing the planning target was recorded as variance and did not weaken acceptance.

The meter remained available, but four stories converged before the next sample. The combined 177,808 reading is retained as trustworthy aggregate telemetry and the method now makes the transition of each story to Accepted the mandatory sampling point.

Sprint 11 completed at an exact final goal reading of 496,163 tokens against its 500,000 target and 800,000 ceiling. Its earlier 453,156 closure-ritual sample remains useful phase telemetry but is not the final goal total.

## Expanded Sprint 12 amendment

The product owner expanded the inactive Sprint 12 recommendation from 29 to 84 points by combining the operable-loop and private ring-game beta milestones.

- Planning target: 1,200,000 tokens
- Runtime ceiling: 1,900,000 tokens
- Checkpoints: 600,000, 1,200,000, and 1,600,000 cumulative tokens
- Target conversion: 14,285.7 tokens per point
- Ceiling conversion: 22,619.0 tokens per point

Sprint 10 and Sprint 11 exact final goal readings average 16,450.9 tokens per point, which scales naively to 1,381,874 tokens for 84 points. A fixed-plus-marginal decomposition estimates 921,563 tokens because Sprint 12 shares one quality gate and review package. The 1,200,000 target carries a 30.2% integration uplift over that shared-overhead estimate; the 1,900,000 ceiling preserves correction headroom.

Because Sprint 12 is materially larger than the two 29-point observations, its pilot result is not directly comparable without segmentation. Closure must report forecast error against both models and must not claim an intrinsic points-to-token conversion.

## Pilot conclusion after Sprint 12

Sprint 12 captured all sixteen story boundaries and remained within one goal from activation through implementation, full gate, process evidence, Ratatui captures, PDF authoring, complete visual inspection, and closure. Exact final usage was 701,101 tokens: 41.6% below the 1,200,000 target with 63.1% of the 1,900,000 ceiling unused. Only the 600,000 checkpoint was crossed, during evidence work after all stories and the final-diff gate passed.

The aggregate meter is sufficiently available and trustworthy for authorization ceilings, cumulative checkpoints, and retrospective phase analysis. The three-sprint sample does not support a stable story-point conversion. Sprint 12's shared gate/evidence overhead and materially larger batch produced a much lower per-point reading than Sprints 10 and 11, while model/reasoning identity and billing-component detail remained unobservable.

Retain the following practice after the pilot:

- Set one explicit goal-level ceiling for an activated sprint.
- Forecast a planning target from comparable phases, integration risk, and evidence cost; use story points only as a weak secondary input.
- Sample every story acceptance and the gate/evidence/closure boundaries.
- Preserve every acceptance and review gate when a target is crossed.
- Reauthorize before exceeding a ceiling; never reset or split a goal to conceal consumption.
- Report aggregate tokens as internal delivery telemetry, not billing or an intrinsic points-to-token conversion.
