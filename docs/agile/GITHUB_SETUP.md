# GitHub Project Setup

The repository includes issue forms and a pull-request template. Creating remote labels or a GitHub Project is an external write and should be done only when explicitly authorized.

## Recommended project fields

| Field | Type | Values |
|---|---|---|
| Status | Single select | Backlog, Ready, In Progress, Review, Done, Blocked |
| Epic | Single select | E0 through E11 |
| Sprint | Iteration | Two-week cadence |
| Points | Number | 1, 2, 3, 5, 8, 13 |
| Priority | Single select | P0 Critical, P1 High, P2 Normal, P3 Low |
| Risk | Single select | Critical, High, Medium, Low |
| Release | Single select | A Core, B Single Table, C Ring Beta, D Tournament, E Public |

## Recommended labels

### Type

- `type:story`
- `type:bug`
- `type:spike`
- `type:chore`
- `type:decision`

### Area

- `area:core`
- `area:protocol`
- `area:server`
- `area:tui`
- `area:lobby`
- `area:persistence`
- `area:tournament`
- `area:operations`
- `area:docs`

### Priority and state

- `priority:p0`
- `priority:p1`
- `priority:p2`
- `priority:p3`
- `blocked`
- `security`
- `privacy`
- `breaking-change`

## Board automation

- New issues enter `Backlog`.
- Issues with acceptance criteria, points, and no unresolved dependency may move to `Ready`.
- Opening a linked pull request moves the issue to `In Progress`.
- Marking a pull request ready for review moves it to `Review`.
- Merging a pull request closes linked issues and moves them to `Done`.
- `blocked` items remain visible and name their blocking decision or dependency.

## Naming conventions

- Story: `[E2.3] Implement short all-in reopening rules`
- Bug: `[BUG] Side-pot eligibility includes folded seat`
- Spike: `[SPIKE] Benchmark table snapshot replay`
- Pull request: `E2.3: enforce full-raise reopening semantics`

## Metrics

Track trends, not individual productivity:

- Sprint-goal success
- Cycle time from In Progress to Done
- Escaped critical/high defects
- Reopened work
- Blocked time by cause
- Automated invariant coverage
- Build and integration-test reliability
- Release recovery and capacity evidence

Do not use story points to rank people or agents.
