# TASK-3: Document the $bindable(null) cascade pattern in CLAUDE.md Known Patterns

## Goal
A new entry in `CLAUDE.md`'s "Known Patterns & Gotchas" section documents the `$bindable(null)` cascade failure mode discovered during the SilencePad regression on 2026-04-23. The entry follows the existing format of that section (one short paragraph, lead with the rule, give a concrete fix) and is specific enough that a future agent encountering the same symptom can recognize it without re-investigating.

## Context
On 2026-04-23, adding `SilencePad` to `VideoOptions` caused a CT test regression: the Advanced section in VideoOptions stopped rendering after its toggle button was clicked. Three consecutive red CI runs followed before the regression closed (`23cc377` → `3fd62b9` → `4a23608`). The first fix attempt — adding a guard inside `$effect` — was insufficient.

**Root cause** (from OBSERVER-STATE Run 10):
> `$bindable(null)` defaults trigger a synchronous `undefined→null` write-back during component initialization — before any `$effect` runs — causing a reactive cascade that can collapse a parent `{#if}` block.

**The fix that worked** (`4a23608`):
- Remove `null` defaults from `$bindable()` declarations in SilencePad
- Initialize `pad_front` / `pad_end` to `null` explicitly in the parent's state (the CT test's `baseOpts`, and presumably real callers)

This pattern has now appeared in two consecutive Observer Loop runs (Run 10, Run 11) without being canonicalized in CLAUDE.md. It is becoming a Known Bug Class by repetition rather than by deliberate documentation. This task closes that gap.

Relevant files:
- `CLAUDE.md` at repo root — the project's main context file. Has a "Known Patterns & Gotchas" section near the bottom with three existing entries (FFmpeg/ImageMagick PATH; `validate_output_name()` requirement; …). New entry appends here.
- `OBSERVER-STATE.md` Run 10 — has the full root-cause analysis. Worker should read this for full context before writing the entry.
- `src/lib/SilencePad.svelte` — the component where the regression originally surfaced. Worker should glance at it to ground the entry in real code.
- The commits in the regression arc: `23cc377` (regression introduced), `3fd62b9` (insufficient guard fix), `4a23608` (correct fix).

## In scope
- `CLAUDE.md` — append one new entry to the existing "Known Patterns & Gotchas" section. Keep style consistent with the surrounding entries (bold lead phrase, then explanation, then concrete fix).

## Out of scope
- Any code change to `SilencePad.svelte`, `VideoOptions.svelte`, or any other component
- Refactoring the existing Known Patterns entries
- Adding entries for any other pattern (only the `$bindable` cascade)
- `KNOWN-BUG-CLASSES.md` — that file is for resolved code bugs with file/commit citations; the `$bindable` cascade is a class-of-bug not tied to a single fix, which makes it Known Patterns territory not Known Bug Classes territory. (If a future occurrence happens, BC-005 may be opened then.)
- Creating a fix recipe for the pattern itself; the entry just names the trap and the workaround

## Steps
1. Read `OBSERVER-STATE.md` Run 10 — specifically the "Fragile / high-risk areas" paragraph that describes the root cause.
2. Read `CLAUDE.md`'s "Known Patterns & Gotchas" section. Note the format: each entry leads with a bold phrase ending in a period, then a paragraph explaining the pattern, then a concrete fix or rule.
3. Read `src/lib/SilencePad.svelte` to ground the entry — confirm what the `$bindable(...)` declarations look like in practice.
4. Append a new entry to "Known Patterns & Gotchas". Lead with a phrase like `**$bindable() defaults inside conditionally-rendered components trigger a write-back cascade.**` then a paragraph describing: what triggers it (a `$bindable(null)` or `$bindable(<default>)` whose parent passes `undefined`), what fails (the cascade can collapse a parent `{#if}` block, removing the entire component subtree), why `$effect` guards don't fix it (initialization write-back fires before any effect runs), and the fix (remove the default from `$bindable()` and initialize the parent's state to a concrete value).
5. Verify the entry fits the section's style — same paragraph length, same imperative voice, same `**bold lead phrase.**` opener.
6. No build needed — this is a docs-only change.

## Success signal
- `grep -n "bindable" CLAUDE.md` returns at least one line in the Known Patterns section.
- The new entry is between 80 and 200 words — not so short it loses information, not so long it dominates the section.
- A future agent who reads only this entry (without OBSERVER-STATE) can: (a) recognize the symptom, (b) name the cause, (c) know the fix without re-investigating. Worker self-checks by re-reading the entry cold.

## Notes
- This is the only Known Patterns entry being added in this session. Don't bundle other observations.
- Don't reference OBSERVER-STATE Run 10 by name in the entry — the entry should stand alone. The CLAUDE.md file is read by every fresh agent; OBSERVER-STATE is project-local context. The entry must work without OBSERVER-STATE present.
- Tone: declarative and brief. Match the existing entries: "FFmpeg and ImageMagick must be in PATH — Tauri does not bundle them." That register.
