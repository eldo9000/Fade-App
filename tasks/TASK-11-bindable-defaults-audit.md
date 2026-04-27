# TASK-11: Audit and remediate `$bindable(<default>)` declarations

## Goal
Five Svelte 5 components currently violate the documented `$bindable()` cascade pattern from `CLAUDE.md`: they declare `$bindable(<default>)` instead of bare `$bindable()`. Each is audited; offending defaults are removed; parents are updated to initialize the bound state to a concrete value. The CLAUDE.md "Known Patterns & Gotchas" entry is no longer being silently violated across the frontend.

## Context
`CLAUDE.md` documents the bug class precisely:

> **`$bindable()` defaults inside conditionally-rendered components trigger a write-back cascade.** When a child declares `$bindable(null)` or `$bindable(<default>)` and the parent passes `undefined` (e.g., an uninitialized options field), Svelte 5 performs a synchronous write-back during component initialization — before any `$effect` runs. That write-back mutates the parent's state, which can collapse the `{#if}` block that conditionally renders the child, removing the entire component subtree. […] Fix: remove the default from `$bindable()` (use bare `$bindable()`) and initialize the parent's state variable to a concrete value (`null`, `0`, etc.) instead of leaving it undefined.

Concern-based analysis (M-3) found five sites:
- `src/lib/Timeline.svelte:6` — `options = $bindable(null)`, `vizExpanded = $bindable(false)` **(highest risk: conditionally rendered)**
- `src/lib/CodecWarning.svelte:2` — `resolution = $bindable()` (already bare — verify, possibly false positive)
- `src/lib/CropEditor.svelte:8-9` — `cropActive = $bindable(false)`, `cropAspect = $bindable(null)`
- `src/lib/AnalysisTools.svelte:7` — `selectedItem = $bindable(null)`
- `src/lib/ChromaKeyPanel.svelte:6` — `selectedItem = $bindable(null)`

`Timeline.svelte` is the most likely real-world offender because it's wrapped in `{#if mediaType === 'video'}` per `App.svelte:2943`. The others may or may not be conditionally rendered — verify per-component.

The cascade is silent: it doesn't crash, doesn't log, doesn't fail tests. It manifests as state that "won't settle" or UI that flickers on first render. Past incident at SESSION-STATUS commit `389ac1d` documented the pattern.

Relevant files:
- `src/lib/Timeline.svelte:1-30`
- `src/lib/CodecWarning.svelte:1-15`
- `src/lib/CropEditor.svelte:1-20`
- `src/lib/AnalysisTools.svelte:1-15`
- `src/lib/ChromaKeyPanel.svelte:1-15`
- `src/App.svelte` — parent that renders each (search for `<Timeline`, `<CropEditor`, etc.)

## In scope
- Per component, in priority order:
  1. Read the component's `$bindable` declarations.
  2. Search `App.svelte` (and any other parent) for the `<Component … />` invocation. Check whether it's inside an `{#if}` block.
  3. If the component is conditionally rendered AND has a `$bindable(<default>)`, change to bare `$bindable()` and initialize the parent's state to the same concrete value the default supplied.
  4. If the component is unconditionally rendered, the cascade still happens but is harmless. Still preferable to use bare `$bindable()` for consistency, but lower priority.
- Manual smoke-test each component in `tauri dev`:
  - Timeline: switch to a video file, confirm options panel renders and stays.
  - CropEditor: enable crop, confirm crop UI persists.
  - AnalysisTools: select an item, confirm selection persists.
  - ChromaKeyPanel: select chroma source, confirm selection persists.

## Out of scope
- Adding tests — Svelte 5 component testing is not currently set up in this repo (Playwright CT is unreliable on macOS 26 beta per project memory).
- Refactoring component prop interfaces beyond the `$bindable` change.
- Auditing components outside the five flagged sites. A repo-wide sweep can be a follow-up.

## Steps
1. Read each of the five component files. Confirm the `$bindable` declarations match the static-analysis report.
2. For each, find the parent invocation in `src/App.svelte`:
   ```bash
   grep -n "<Timeline\|<CodecWarning\|<CropEditor\|<AnalysisTools\|<ChromaKeyPanel" src/App.svelte
   ```
3. For each invocation, determine the conditional-rendering context. Walk upward through nested blocks to find any `{#if}` ancestor.
4. For each component that is conditionally rendered AND has `$bindable(<default>)`:
   a. Change `let { foo = $bindable(<default>), ... } = $props()` to `let { foo = $bindable(), ... } = $props()`.
   b. In the parent, locate the state declaration that backs the `bind:foo` prop. Initialize it to the concrete value the default supplied. E.g.:
      - Was: `let timelineOptions = $state();` → `let timelineOptions = $state(null);`
      - Was: `let cropActive = $state();` → `let cropActive = $state(false);`
5. For each unconditionally-rendered component with a `$bindable(<default>)`, leave alone for now — the cascade exists but is benign. Mark it in the SESSION-STATUS Known Risks for future sweep.
6. Manual smoke per the "In scope" list. Note any UI that no longer renders correctly.
7. `npm run check` (Svelte typecheck), if available.

## Success signal
- `grep -n "\\\$bindable(" src/lib/Timeline.svelte src/lib/CropEditor.svelte src/lib/AnalysisTools.svelte src/lib/ChromaKeyPanel.svelte` shows bare `$bindable()` for the conditionally-rendered cases (or documented justification for any remaining `$bindable(<default>)`).
- `npm run check` exits 0.
- Manual smoke confirms each component still mounts and persists state correctly.

## Notes
- `CodecWarning.svelte:2` per the concern-based pass already uses bare `$bindable()` — verify this in the file and skip if true.
- The CLAUDE.md fix recipe is concrete: child uses bare `$bindable()`, parent initializes to concrete value. Don't introduce `$effect` guards — they fire after initialization, too late.
- If a smoke test fails after the change, the most likely cause is the parent's state initialization not matching the prior default. Read the parent carefully.
- This is documented bug-class hygiene, not a bug-fix per se. No reproducer is currently failing — the goal is to prevent the next one.
