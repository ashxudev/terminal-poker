# Agent Delivery Calibration

Last updated: 2026-09-08

## Purpose

Story points remain relative measures of complexity, uncertainty, and validation effort. This record adds separate empirical agent-clock and token-consumption forecasts so sprint plans can expose expected active execution time, fixed review overhead, unplanned rework, and bounded agent consumption without redefining points as time or tokens.

Active clock excludes gaps between user-triggered work blocks. It includes implementation, tests, report generation, mandatory visual inspection, corrections, and sprint closure performed inside those blocks.

## Observed Sprints 1-12

| Sprint | Accepted points | Active clock | Seconds per point | Calibration note |
|---:|---:|---:|---:|---|
| 1 | 18 | 15:55 | 53.0 | Reasonable initial calibration |
| 2 | 15 | 38:45 | 155.0 | First PDF pipeline plus 9:43 of Ratatui correction made the estimate materially low |
| 3 | 50 | 25:46 | 30.9 | Rules uncertainty was materially higher than the realized agent clock |
| 4 | 26 | 19:04 | 44.0 | Stable |
| 5 | 26 | 19:40 | 45.4 | Stable |
| 6 | 24 | 21:30 | 53.8 | Stable, with authorization and subscription integration costing slightly more per point |
| 7 | 26 | 23:43 | 54.7 | Stable; one compact-pot visual correction and a full post-fix release gate remained inside the control range |
| 8 | 34 | 36:05 | 63.7 | New process seam found one stream-cursor defect; release linking, PDF correction, and full ritual closure remained inside the control range |
| 9 | 29 | 21:38 | 44.8 | Adapter proof and snapshot/reconcile lifecycle converged quickly; one rules defect and renderer cross-check remained below forecast |
| 10 | 29 | 31:34 | 65.3 | Multi-table routing and public lobby converged quickly; portable review-runner correction, full-width PDF redesign, token-report standards correction, and final gate rerun remained within the 28-42 minute range |
| 11 | 29 | 40:51 | 84.5 | Atomic storage/restart plus reconnect-lock and stale-build-label corrections required two complete final gates and regenerated optimized evidence; still inside the 32-48 minute range |
| 12 | 84 | 1:14:19 | 53.1 | Combined operable-loop/private-beta milestone; capacity found one admission defect, while one shared gate and evidence package amortized fixed work |
| 13 | 39 | 4:53:23 | 451.4 | Formal 2h15 qualification was actively monitored; retained failure/correction plus production evidence and PDF QA dominated clock |
| **Total** | **390** | **6:08:50** | **56.7** | Includes implementation, corrective work, production evidence, PDF, complete inspection, and closure rituals |

Without the 9:43 Sprint 2 Ratatui correction, the twelve-sprint total is 5:59:07, or 55.2 seconds per point. Sprints 4-12 form the most recent simple-rate sample: 307 points in 4:48:24, or 56.4 seconds per point.

## Forecast model

The Sprints 3-9 observations continue to support this provisional model:

```text
expected active sprint clock = 15m 50s fixed sprint/review overhead
                             + 16.4 seconds x committed points
```

Sprint 12's model forecast was 38:48 versus an exact 1:14:19 goal clock, a 35:31 or 91.5% underestimate. The result was 0:41 below the conservative 75-120 minute control range because one shared gate/review package amortized fixed work across 84 points. The simple aggregate rate remains directionally useful, but the fixed-plus-linear model persistently underweights process evidence, visual review, and corrective integration. Retire it as the primary forecast after Sprint 12; future ranges should combine the recent simple rate with explicit phase allowances for integration, full gate, process evidence, visual QA, and closure.

The clock forecast is a control signal, not a stop condition. At the upper bound, record the cause, update the remaining forecast, and continue while the sprint goal remains authorized and achievable.

## Token-budget pilot

The accepted policy is defined in [Sprint Token Budget Policy](TOKEN_BUDGET_POLICY.md). Sprints 10-12 will collect comparable goal-level observations before this record claims a predictive points-to-token model.

Sprint 9 supplies the only retained baseline:

| Sprint | Accepted points | Tracked tokens | Goal clock | Tokens per point | Tokens per goal minute | Note |
|---:|---:|---:|---:|---:|---:|---|
| 9 | 29 | 366,526 | 21:49 | 12,638.8 | 16,800.3 | Unbudgeted observational baseline; goal clock was 11 seconds above the agile active clock |
| 10 | 29 | 457,988 | 31:34 | 15,792.7 | 14,508.6 | First budgeted observation; 1.8% above target and 36.8% below ceiling; one combined-story sampling anomaly |
| 11 | 29 | 496,163 | 43:54 | 17,109.1 | 11,302.1 | Second budgeted observation; exact final goal total 0.8% below target and 38.0% below ceiling; every story boundary sampled |
| 12 | 84 | 701,101 | 1:14:19 | 8,346.4 | 9,434.0 | Exact final goal; 41.6% below target and 63.1% of ceiling unused; every boundary sampled |

One observation is not a reliable forecast. Token consumption can change with model and reasoning configuration, carried context, compaction, test and tool output, image/PDF inspection, defects, and corrective rework. Aggregate goal tokens are an internal consumption measure, not a billing estimate.

Sprint 10 pilot result:

- Planning target: 450,000; closure checkpoint 457,988, or 7,988 (1.8%) over target.
- Runtime ceiling: 725,000; headroom 267,012 (36.8%).
- Cumulative readings: activation 0; coherent four-story boundary 177,808; pre gate 187,803; post gate 197,843; before review evidence 275,172; after first final PDF inspection 337,587; closure checkpoint 457,988.
- Phase deltas where readings permit: implementation/coherent stories 177,808; stabilization to pre-gate 9,995; first full gate 10,040; optimized rebuild/capture to evidence boundary 77,329; initial screenshot/PDF production and complete inspection 62,415; final gate rerun, rituals, full-width/token-report corrections, synchronization, and closure checkpoint 120,401.
- Visibility: aggregate meter remained available and trustworthy. Per-story granularity was missed because four stories converged before the next reading; the delivery method now requires sampling at each acceptance transition.
- Comparison: 91,462 tokens (+25.0%) above Sprint 9 total, 3,153.9 tokens/point (+25.0%), and 2,291.7 tokens/minute (-13.6%). Context, process/PDF corrections, large full-gate output, and visual inspection make causal attribution approximate.

Sprint 11 recommendation:

- Planning target: 500,000 tokens
- Runtime ceiling: 800,000 tokens
- Checkpoints: 325,000, 500,000, and 675,000
- Basis: Sprint 10 target adherence plus additional atomic storage, corruption, and forced-restart uncertainty; this remains a planning envelope, not a points-to-token model

Sprint 11 pilot result:

- Planning target: 500,000; exact final goal reading 496,163, or 3,837 (0.8%) below target.
- Runtime ceiling: 800,000; headroom 303,837 (38.0%).
- Cumulative readings: activation 0; E7.7 140,253; E8.3a 203,320; E8.4a 227,806; E11.1c 290,924; post first full gate 312,874; before PDF evidence 376,757; after final PDF inspection 395,037; closure ritual 453,156; final goal closure 496,163.
- Phase deltas: E7.7/activation and seam work 140,253; E8.3a 63,067; E8.4a 24,486; E11.1c 63,118; first full gate 21,950; corrected final gate/optimized evidence preparation 63,883; PDF authoring/render/inspection 18,280; ritual/calibration/handoff synchronization 58,119; final consistency audit and goal close 43,007.
- Visibility: all mandatory story samples were captured; the 325,000 checkpoint was crossed only during evidence work after stories and final-diff gate acceptance.
- Comparison with Sprint 10: 38,175 more tokens (+8.3%), 1,316.4 more tokens per point (+8.3%), and approximately 3,206.5 fewer tokens per goal minute (-22.1%).

Sprint 12 recommendation:

- Expanded commitment: 84 points combining the 29-point operable loop and 55-point private ring-game beta
- Naive proportional forecast: 1,381,874 tokens at the Sprint 10-11 exact-final average of 16,450.9 tokens per point
- Fixed-plus-marginal forecast: 921,563 tokens, using approximately 242,710 fixed tokens plus 8,081.6 marginal tokens per point
- Planning target: 1,200,000 tokens, 30.2% above the shared-overhead forecast
- Runtime ceiling: 1,900,000 tokens
- Checkpoints: 600,000, 1,200,000, and 1,600,000
- Basis: one shared gate/review lowers fixed duplication, while policy, credentials, histories, admission races, load, and beta integration materially increase correction risk

At Sprint 12 closure, compare both forecast forms and segment the 84-point milestone sprint from the two prior 29-point sprints. Fit a general token forecast only if the fixed/marginal decomposition is supported; otherwise extend or retire the pilot.

Sprint 12 pilot result:

- Planning target: 1,200,000; exact final usage 701,101, or 498,899 (41.6%) below target.
- Runtime ceiling: 1,900,000; headroom 1,198,899 (63.1%).
- Cumulative readings: activation 0; all sixteen story transitions 38,364 through 494,593; before gate 512,339; after gate 522,747; after source inspection 561,125; after final PDF inspection 632,922; closure ritual 672,991; final pre-close audit 691,232; exact final goal 701,101.
- Forecast comparison: 220,462 (23.9%) below the 921,563 fixed-plus-marginal forecast and 680,773 (49.3%) below the 1,381,874 naive proportional forecast at exact closure.
- Visibility: every mandatory story sample was captured. The 600,000 checkpoint was crossed only in PDF evidence work after implementation and final-gate acceptance; remaining work was reforecast below 750,000 with no acceptance risk.
- Conclusion: aggregate goal budgets are feasible for ceilings, checkpoints, and phase telemetry. The sample rejects a general points-to-token conversion because fixed work, shared evidence, model/context state, defects, and tool output dominate comparability.

Post-pilot recommendation:

Sprint 13 budget result: exact goal usage was 1,087,084 tokens against a 750,000 target and 1,100,000 ceiling: 337,084 (44.9%) above target with 12,916 (1.2%) ceiling headroom. Exact goal clock was 4:53:23 and usage was 27,873.9 tokens/point. The meter remained available and every story/gate/evidence/closure boundary was sampled. The variance was dominated by an honestly retained formal-soak failure, accelerated correction, continuous 2h15 qualification monitoring, strict final-diff correction, and two PDF visual-QA corrections; no gate or scope was reduced.

- Continue explicit planning targets and authorization ceilings only for sprints where the product owner accepts them.
- Base targets on comparable delivery phases and risk reserves; use points only as a weak secondary input.
- Preserve story/gate/evidence/closure sampling and never weaken Definition of Done for a target or ceiling.
- Re-open model fitting only after several comparable scopes run under an observable stable model/reasoning configuration.

## Rebased Sprint 14 recommendation

The installed-shell tangent had no sprint clock or runtime goal, so it is not a
new throughput or token-calibration observation. Its implementation is retained
as acceptance-pending and its uncertainty informs the next phase envelope only.

- Commitment: 40 points for installed shell, UI platform, local profile,
  repeatable Quick Practice, shared results/help, and cross-shell evidence.
- Active-clock control range: 65-105 minutes, with UI dependency/motion,
  migration, installed process, human usability, PDF, and visual-QA allowances.
- Token planning target: 750,000.
- Runtime ceiling: 1,150,000.
- Checkpoints: 400,000, 750,000, and 975,000.
- Basis: comparable integrated client/process/review phases from Sprints 8-12,
  with additional terminal-compatibility and visual correction reserve. Points
  remain only a weak secondary input.

Sprint 14 result:

- Commitment: 40/40 accepted after the product-owner retest.
- Active goal clock: 4,358 seconds (72:38), inside the 65-105 minute control
  range.
- Exact final runtime reading: 986,494 tokens, 236,494 (31.5%) above the
  750,000 planning target and 163,506 (14.2%) below the 1,150,000 ceiling.
- Rates: 24,662.4 tokens per accepted point and approximately 13,581 tokens per
  active minute.
- Visibility: all planned story/gate/evidence checkpoints were recorded. The
  meter froze at 986,494 while the goal waited for the human retest, so closure
  adds no artificial post-review consumption.
- Variance cause: two product-owner review failures caused a raise/sweep/color/
  rollover correction, replacement of the interstitial Results screen with the
  table console, an award-notification correction found by visual inspection,
  repeated installed journeys, and three complete PDF render/inspection passes.
  Scope and quality gates were not weakened.
- Forecast implication: retain separate interaction-discovery allowance before
  full PDF evidence in UI-heavy sprints; do not infer that 40 points generally
  cost 986,494 tokens.

## Sprint 15 D1 recommendation

- Commitment: 76 points for tournament-critical private Host/Join plus the
  complete functional single-table tournament milestone. Custom Practice and
  public Join/reach move behind D1.
- Active-clock control range: 120-210 minutes; low confidence.
- Token planning target: 675,000.
- Runtime ceiling: 750,000, as explicitly directed by the product owner.
- Checkpoints: every story; 200,000; 400,000; 550,000; pre/post full gate;
  650,000; pre-review; 700,000; post-PDF/visual QA; closure.
- Comparable evidence: Sprint 12 delivered 84 points under one shared gate for
  701,101 tokens; Sprint 14's 986,494-token corrective UI path shows that human
  review rework can overwhelm a narrow ceiling. The recommendation therefore
  removes noncritical Custom Practice and public reach, front-loads a control
  walkthrough, and treats the 750,000 ceiling as high risk.
- The budget is not a points conversion or a guarantee. No acceptance, privacy,
  process, PDF, or visual-inspection gate may be waived. Forecast breach stops
  before the ceiling and requires explicit reauthorization.

## Sprint 15 result

- Commitment: 76/76 accepted after the product-owner "would play again" verdict.
- Original goal ceiling: 750,000. The runtime later reported 777,427 because
  accounting continued after automatic budget limiting.
- First completion goal ceiling: 50,000; final observed reading 73,560 after
  context hydration and controller-release retries, with no accepted delivery
  attributed to it.
- Second completion goal ceiling: 100,000; exact final reading 97,271 and 838
  seconds for the atomic-publication correction, clean gate, exact installation,
  artifact synchronization, and acceptance handoff.
- Aggregate associated-goal usage: 948,258 tokens and 4,928 active seconds
  (82:08), or 12,477 tokens per accepted point and approximately 11,546 tokens
  per active minute. This is 273,258 (40.5%) above the 675,000 planning target.
  The apparent 48,258-token excess over combined authorized ceilings is a
  post-limit accounting/controller anomaly: the first two meters continued
  increasing after they entered `budgetLimited` and delivery had stopped.
- Preserve all associated readings; do not treat the completion goals as a reset
  or use this fragmented observation as a direct points-to-token conversion.
- Forecast lesson: native release linking and installed smoke belong before PDF
  work, and a repeatedly failing concurrency test must be investigated as a
  defect rather than budgeted as flake tolerance.

## Sprint 16 result

- Commitment: 13/13 accepted for the one-renderer responsive portrait table and
  its installed/visual evidence boundary.
- No token budget or ceiling was requested. The unbudgeted goal's final
  reading was 405,502 tokens; retain this as telemetry only, not authorization
  or a points conversion.
- Active goal clock: 3,436 seconds (57:16), with no recorded inter-sprint gap;
  264.3 seconds per accepted point, approximately 31,192 tokens per accepted
  point, and approximately 7,081 tokens per active minute.
- Activation omitted the required active-clock forecast, so forecast variance
  cannot be calculated. Future sprint activation must record its clock range
  before implementation starts.
- Corrective rework: one missing pending-authority notice found by the full
  gate, clipped hero cards found by screenshot inspection, and caption spacing
  found by first-pass PDF inspection. All were corrected and the relevant full
  gates were repeated before acceptance.
- Forecast lesson: preserve dedicated visual-correction allowance and add
  height-sensitive semantic screenshot assertions before artifact generation.

## Planning rules

- Preserve Fibonacci points for relative scope and risk.
- Add an active-clock forecast and range to every recommendation and activated sprint.
- During the Sprints 10-12 pilot, add a separate token target, runtime ceiling, and checkpoint schedule to every recommendation and activated sprint.
- Split 13-point stories before activation when two independently reviewable risk boundaries exist.
- Include the mandatory PDF, actual-Ratatui capture, complete-hand continuity, and page-by-page visual QA in the fixed clock allowance and Definition of Done.
- Record corrective rework separately from planned delivery clock.
- At sprint closure, record trigger-to-handoff active time, excluded inter-sprint gaps, variance, and its cause.
- At budgeted sprint closure, record final tokens, phase checkpoint deltas, tokens per accepted point, tokens per active minute, target/ceiling variance, configuration, and meter anomalies.
- Recalibrate after every three completed sprints or after a material change in tooling, architecture, or evidence requirements.

Do not use these agent-clock or token observations as human engineering estimates, production release forecasts, or exact billing data.

## Sprint 17 - showdown rules

26/26 new points; approximately 48 active minutes (02:02-02:50 UTC, 8 September
2026), versus forecast 75-150 minutes. No token budget requested; aggregate token
usage unavailable. Scope reused the existing authority and renderer. Corrections
covered timer-driven hand rollover, shutdown history finalization, shared hand
evaluation, and a clipped minimum-width hero label. Final evidence includes 295
passing tests, three installed shell smokes, and nine individually inspected PDF
pages. Keep timer-driven completion and minimum-width visual review in future
forecasts; do not equate this one sprint's points with elapsed minutes.
