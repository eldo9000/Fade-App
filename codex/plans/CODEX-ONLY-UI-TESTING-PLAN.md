# Codex-Only UI Testing Plan

## Intent
Use Codex as the primary testing/verification agent for UI bug discovery, reproduction, and fix verification.

This path does not depend on Anthropic computer-use APIs.

## Core Principles
- Prompt-driven missions are translated into reproducible test runs.
- Every finding must include reproduction steps and evidence.
- Codex can patch focused UI bugs when explicitly authorized.
- Existing tests stay intact; Codex adds a layered system on top.
- Prompt missions should name the exact control to click, the exact file scope, and the exact stop condition.
- Prompt missions should not rely on renaming, suffix generation, or other cleanup work unless the mission explicitly asks for it.
- If the requested control is missing or the click does nothing, stop and report the UI state instead of improvising.

## System Lanes

### Lane 1: Deterministic Matrix Lane
- Source: row-based matrix (CSV-first; Excel-compatible).
- Purpose: explicit row coverage and release confidence.
- Execution: scripted runs that mark each row pass/fail/blocked.

### Lane 2: Prompt Mission Lane
- Source: natural-language mission request.
- Purpose: exploratory and directed bug hunting in UI behavior.
- Execution: Codex maps mission goals into a runbook and automated checks.

## Codex-Native UI Interaction Strategy
Primary engines:
- Playwright component tests (existing, expanded).
- App workflow scripts that emulate user interactions and state transitions.
- Targeted regression scripts for known bug classes.

Supported interaction classes in this model:
- click, double-click, type, key chords, tab navigation, toggles
- drag-and-drop simulation in automation frameworks where supported
- slider/trim/selection style interactions via pointer events in test runners

Not part of this model:
- direct freehand control of host macOS mouse pointer from this Codex chat session.

## Mission Lifecycle
1. Receive mission prompt.
2. Convert prompt to mission spec (scope, constraints, stop criteria).
3. Execute focused sweep.
4. Emit findings with severity and evidence.
5. If authorized, patch targeted bug(s).
6. Re-run focused regression checks.
7. Publish verification report.

## Required Mission Inputs
- Target area (example: MP4 tab/video options).
- Goal (example: convert MP4 -> WebM and find UI state bugs).
- Constraints (timebox, no refactors, allowed files).
- Completion criteria (what counts as done).
- Exact UI action for the conversion step when one is required.
- Output policy: whether names may be changed, or whether the folder is assumed to be clean.

## Output Contract
Each run produces:
- run summary (pass/fail/blocked)
- finding list with repro steps
- artifacts/log references
- patch verification status (if fixes were applied)

## MP4 Pilot (Phase 1)
Pilot objective:
- Prove prompt-driven mission execution for MP4 workflows.

Pilot mission examples:
- "Convert sample MP4 to WebM and report every UI inconsistency encountered."
- "Stress codec + resolution + fps controls and identify reset/state bugs."

Pilot success criteria:
- At least one mission completes end-to-end with reproducible report.
- At least one bug is validated and re-tested after fix (if authorized).
- Repeat run shows stable results and clear diff from prior run.

## Repository Layout (Codex-owned)
- `codex/plans/` architecture and rollout docs
- `codex/notes/` mission specs and runbooks
- `codex/reports/` findings and verification reports
- `codex/artifacts/` exported artifacts and machine-readable summaries
- `codex/logs/` execution logs

## Initial Build Order
1. Define mission template and report template.
2. Define MP4 pilot mission set.
3. Wire command entry points for repeatable runs.
4. Add matrix source + row executor.
5. Add scheduled sweep cadence.

## Guardrails
- No destructive edits outside requested scope.
- No replacement of existing test suites; only extension.
- Keep Claude-focused feature workflow separated from Codex testing workflow.
