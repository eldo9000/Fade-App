# Codex Testing System V1

## Why this exists
This plan is for a Codex-owned testing loop that reduces manual copy/paste and repeated repro work.

Primary goal:
- Catch UI and conversion regressions quickly with reproducible evidence.

Secondary goal:
- Let Codex run click-driven exploration, identify bugs, and fix targeted UI issues in the same loop when requested.

## Scope
This system has two complementary test modes.

## Mode A: Matrix Catalog (Deterministic Coverage)
Purpose:
- Ensure broad, explicit coverage across formats, options, and known risk combinations.
- Function like an "Excel sheet" where every row is checked and accounted for.

How it works:
- A test matrix defines one scenario per row.
- Each row includes: case id, area, setup, action, expected result, severity if failed, and evidence links.
- Runner executes rows and records pass/fail per row.

Expected result:
- Full-row accountability: no hidden gaps, no silent skips.
- Repeatable sweep output showing exactly what passed, failed, and changed.

## Mode B: Prompt-Driven Agent Testing (Exploratory + Directed)
Purpose:
- Support language-driven requests like: "Go mess around with MP4 tab and fix UI bugs."
- Find behavior bugs that strict scripted checks may miss.

How it works:
- A prompt defines a mission, target area, and stop conditions.
- Codex performs UI interaction sweeps (click/type/toggle/switch tabs), captures findings, and prioritizes reproducible defects.
- When authorized, Codex patches bugs directly, re-runs focused checks, and reports verification evidence.

Expected result:
- Fast turnarounds on real UI pain points.
- Tight find -> fix -> verify loop on click-driven issues.

## Combined Model
Mode A and Mode B are both required.

Mode A provides:
- Coverage certainty.
- Release confidence.
- Trend metrics over time.

Mode B provides:
- Discovery depth.
- Human-like interaction probing.
- Practical bug-fix throughput for UI behavior.

## Operating Contract for Codex
- Prefer tests over assumptions.
- Every meaningful claim must include reproducible evidence.
- Track all findings with severity, repro steps, and verification status.
- Preserve prior test systems; extend without destructive replacement.
- Work from Codex-owned docs/artifacts first, then integrate with existing project systems intentionally.

## Ownership and Boundaries
- `CLAUDE.md` remains intact unless explicitly requested.
- Existing testing work is not removed.
- Codex may edit application code when explicitly requested for bug fixing and verification loops.
- Codex should keep planning/reporting assets in `codex/` so responsibilities stay clear.

## Evidence Standard
Every failed or fixed issue should include:
- Reproduction steps.
- Expected vs actual behavior.
- Environment/context.
- Command or test run reference.
- Artifact path (logs/screenshots/report).

## Initial Deliverables
1. Matrix schema draft with required columns.
2. Prompt mission template for exploratory sweeps.
3. Unified run summary format for both modes.
4. Bug report template tuned for UI interaction issues.
5. Pilot scope focused on MP4/video-option workflows.

## Definition of Success (V1)
- Matrix run can account for every declared test row.
- Prompt-driven run can execute a mission and produce reproducible findings.
- At least one end-to-end cycle is demonstrated: discover -> patch -> verify -> report.
- Time spent on manual repro/copy-paste drops materially.

## Next planning step
Turn this into an execution plan with:
- file locations,
- schema details,
- run commands,
- report outputs,
- phased rollout.
