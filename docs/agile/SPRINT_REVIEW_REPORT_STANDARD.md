# Sprint Review PDF Report Standard

Applies to Sprint 2 and every later sprint review.

A sprint cannot be marked reviewed or closed until its human-readable PDF report exists, contains the required visual evidence, and the exact final PDF has passed page-by-page visual inspection.

## Required output bundle

For Sprint `N`, produce:

```text
docs/agile/reports/sprint-N/
  report-source.md
  hand-trajectory.md
  visual-qa.md
  screenshots/
    01-...
    02-...
output/pdf/
  sprint-N-review-report.pdf
```

The PDF is the review deliverable. Markdown files are its auditable source, trajectory manifest, and QA record. Screenshots are retained as source evidence.

## Report contents

Use this order:

1. Cover: sprint number, goal, result, review date, build identifier, and one-sentence outcome.
2. Executive summary: what changed, why it matters, and what remains incomplete.
3. Key updates: concise explanations with screenshots from the running product or executable evidence.
4. One-hand trajectory: one complete hand from its initial state to its terminal outcome.
5. Acceptance and quality evidence: stories, tests, invariants, and relevant security, recovery, or capacity results.
6. Decisions and risks: accepted choices, assumptions, known limitations, and release impact.
7. Forecast: recommended next sprint goal without activating it.

For a token-budgeted sprint, the acceptance/quality or forecast section also reports the planning target, runtime ceiling, final tracked tokens, tokens per accepted point, tokens per active minute, material checkpoint variance, and any meter anomaly. Aggregate tokens must not be labelled as exact cost.

The report must be understandable without reading repository Markdown or tool logs.

## Screenshot rules

- Use screenshots of the running build, test harness, trace viewer, or other real executable evidence. Do not use mockups as delivery proof.
- Include at least two clearly captioned screenshots that depict the sprint's key updates.
- Screenshots must be sharp and legible at 100% page zoom, with consistent cropping and no unrelated desktop content.
- Use the same theme, terminal dimensions, font size, and capture scale throughout a trajectory.
- Captions state what changed and what the reader should notice.
- Show only player-authorized and public information. Do not expose deck order, random state, credentials, reconnect tokens, or another player's hidden cards before legitimate reveal.
- A screenshot may serve both the key-update section and the hand trajectory when it genuinely demonstrates both.

## Single-hand trajectory

Every report contains one complete, internally consistent hand of play. It is a narrative and an audit trail, not a collage of convenient states.

Required identity:

- Build identifier
- Table or review-fixture identifier
- Stable hand identifier, or hand number plus deterministic review seed until `HandId` exists
- Seat-to-controller mapping
- Initial stacks and blinds

Required coverage:

1. Initial deal or preflop state
2. At least one meaningful action or sprint-specific state transition
3. A later action or street when the hand reaches one
4. Terminal fold, showdown, or all-in outcome with awarded pot and final stacks

If the selected hand ends early, show enough sequential action frames to make the complete path clear. Never substitute frames from another hand merely to show more streets.

The accompanying `hand-trajectory.md` contains a continuity ledger:

| Step | Screenshot | Phase | Actor | Accepted action/event | Board | Pot | Stacks/contributions |
|---:|---|---|---|---|---|---:|---|

Every screenshot must reconcile with the previous row. The ledger records any automatic blind, street, timeout, or award transition between frames. Test-only deterministic seeds may be reported; production random state must never be exposed.

## Layout quality

- Use A4 or US Letter consistently with at least 18 mm margins.
- Use a readable sans-serif body face at 10 pt or larger and a clear heading hierarchy.
- Prefer short paragraphs, small tables, and one or two screenshots per page.
- Keep screenshot text large enough to read without magnification.
- Use high-contrast colors, consistent captions, page numbers, and a restrained visual palette.
- Avoid clipped text, split captions, overlapping elements, stretched images, dense full-width logs, blank pages, and orphan headings.
- Use ASCII hyphens in generated PDF text to avoid font substitution defects.

## Mandatory visual inspection

Visual inspection happens after the draft review PDF is generated and before the sprint review is accepted.

1. Reopen the generated PDF and confirm its page count and metadata.
2. Render every page to PNG at review resolution using Poppler, normally:

   ```text
   pdftoppm -png -r 150 output/pdf/sprint-N-review-report.pdf tmp/pdfs/sprint-N-review/page
   ```

3. Inspect every rendered page visually, not only through extracted text.
4. Check typography, margins, clipping, overlap, table flow, screenshot sharpness, captions, page numbers, and section transitions.
5. Check the hand screenshots and ledger for identity and state continuity.
6. Fix every defect, regenerate the PDF, rerender all pages, and inspect the new complete render.
7. Reopen the final PDF and perform a text-extraction sanity check for required headings and missing content.
8. Calculate the final PDF SHA-256 and record it in `visual-qa.md` so the inspection is bound to the delivered artifact.
9. Remove temporary page renders after the QA record is complete. Retain report screenshots.

The inspector records every page as Pass or records a defect and its resolution. "Generated successfully" is not visual inspection.

## Review acceptance gate

The sprint review is accepted only when all are true:

- The final PDF exists at the canonical path.
- The report contains the required sections and at least two key-update screenshots.
- One complete hand and continuity ledger use one consistent hand identity.
- Screenshot privacy and provenance checks pass.
- Every final PDF page has a recorded visual Pass.
- `visual-qa.md` contains the exact final PDF SHA-256.
- Automated quality evidence and residual risks agree with the living sprint artifacts.
- A budgeted sprint's final token telemetry agrees with its sprint record and delivery calibration.
- The final handoff links the PDF.

If rendering, capture, or visual inspection cannot be completed, the review remains `Review` or `Blocked`; the sprint must not be marked Done.

## Historical reviews

Sprint 0 and Sprint 1 predate this standard and remain Markdown-only historical records. They are not evidence that a future sprint may omit the PDF gate.
