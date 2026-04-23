# TASK-5: Apply unified polish patterns to FormatPicker.svelte

## Goal
`src/lib/FormatPicker.svelte` renders with the same canonical patterns as the
MP4 panel: shared segmented-button helper (if applicable), canonical spacing,
shrunk button heights, new active-button style, custom checkboxes in gray
tiles. This is the smallest sweep. No behavioral changes.

## Context

Fade is a Tauri 2 + Svelte 5 media-conversion app. `FormatPicker.svelte` is
a generic options panel used by Document, Timeline, and a few other
categories (see usage in `src/App.svelte` — it's called with `formats={...}`
and bound `options`). Because it's used by several categories, changes here
propagate to multiple output types at once.

**Canonical primitives already in `src/app.css`:**
- `.seg-active`, `.seg-inactive`, `.fade-check`, `.fade-range`, `--surface-hint`

**Canonical class strings** (match `VideoOptions.svelte`):
- Segmented row button: `px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative` + `rounded-l-md`/`rounded-r-md` + `-ml-px`
- Vertical segmented button: `w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative` + `rounded-t-md`/`rounded-b-md` + `-mt-px`
- Active: `'seg-active z-10'`
- Inactive: `'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'`
- Form root spacing: `space-y-3` (FormatPicker currently uses `space-y-5` — confirm and migrate)
- Vertical list wrapper: `inline-flex flex-col`
- Checkbox tile: `<label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px] bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2 {checked ? 'text-[var(--text-primary)]' : 'text-white/75'}">` with `<input type="checkbox" class="fade-check">`

TASK-1 put the canonical `seg()` / `segV()` in `src/lib/segStyles.js`.

**Deferred (flag in summary):** slider labels, dropdown conversions, any
bespoke controls that don't fit segmented helpers.

## In scope
- `src/lib/FormatPicker.svelte` only

## Out of scope
- Every other `.svelte` file
- `src/app.css`
- Behavior / schema changes
- Adding slider labels
- Dropdown conversions
- Changing how `FormatPicker` is invoked (no prop changes)

## Steps

1. Read `src/lib/FormatPicker.svelte` end-to-end. Inventory controls: which
   segmented buttons exist, whether any checkboxes / sliders are present.

2. If the file has a local `seg` / `segV` helper, replace with an import from
   `./segStyles.js`. If it uses inline classes directly, migrate call sites
   to use the shared helper.

3. Change the form-root `space-y-*` to `space-y-3`.

4. Any `<div class="flex flex-col">` wrapping a vertical segmented list →
   `<div class="inline-flex flex-col">`.

5. Checkboxes → `fade-check` + canonical tile.

6. Range sliders (if any) → `fade-range` + inline `--fade-range-pct` style.

7. Migrate any inline active class strings to `'seg-active z-10'` via the
   helper.

8. Swap any inline `color-mix(... #000 40%)` "hint panel" backgrounds to
   `var(--surface-hint)` where intent matches.

9. `npx vite build`. Confirm exit 0.

10. Visual spot-check: in the running dev app, pick a Document category
    output (one of the formats routed through FormatPicker — e.g. TXT, RTF,
    DOCX depending on what's wired up) and compare visual parity with MP4.
    Also confirm the same panel layout looks correct across the other
    categories that use FormatPicker (Timeline, etc) — reopen a couple.

## Success signal
- No inline active-button class strings remain.
- Checkboxes (if any) use `fade-check` in `bg-[var(--surface-hint)]` tiles.
- Sliders (if any) use `fade-range` + pct style.
- `space-y-3` on form root, `py-[5px]` on segmented buttons.
- `npx vite build` succeeds.
- Visual parity with MP4 on shared controls, across every category that uses
  FormatPicker.
- Summary lists anything deferred.
