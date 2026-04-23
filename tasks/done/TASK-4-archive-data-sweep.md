# TASK-4: Apply unified polish patterns to ArchiveOptions + DataOptions

## Goal
`src/lib/ArchiveOptions.svelte` and `src/lib/DataOptions.svelte` both render
with the same canonical patterns as the MP4 panel: shared segmented-button
helper, canonical spacing, shrunk button heights, new active-button style,
custom checkboxes in gray tiles. These two files are small enough to batch.
No behavioral changes.

## Context

Fade is a Tauri 2 + Svelte 5 media-conversion app. The MP4/Video panel was
polished first and is the canonical reference. This task sweeps two of the
smaller option panels.

**Canonical primitives already in `src/app.css`:**
- `.seg-active`, `.seg-inactive`, `.fade-check`, `.fade-range`, `--surface-hint`

**Canonical class strings** (match `VideoOptions.svelte`):
- Segmented row button: `px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative` + `rounded-l-md`/`rounded-r-md` + `-ml-px`
- Vertical segmented button: `w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative` + `rounded-t-md`/`rounded-b-md` + `-mt-px`
- Active variant string: `'seg-active z-10'`
- Inactive variant string: `'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'`
- Form root spacing: `space-y-3`
- Vertical list wrapper: `inline-flex flex-col` (shrinks to longest row)
- Checkbox tile: `<label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px] bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2 {checked ? 'text-[var(--text-primary)]' : 'text-white/75'}">` with the inner `<input type="checkbox" class="fade-check">`

TASK-1 put the canonical `seg()` / `segV()` in `src/lib/segStyles.js`.

`ArchiveOptions.svelte` currently has its own local `seg()` helper (line 7).
`DataOptions.svelte` may or may not — inspect and only replace if present.

**Deferred (flag in summary):** slider labels, dropdown conversions, any
bespoke controls that don't fit the segmented helpers.

## In scope
- `src/lib/ArchiveOptions.svelte`
- `src/lib/DataOptions.svelte`

## Out of scope
- Any other `.svelte` file
- `src/app.css`
- Behavior / schema changes
- Adding slider labels
- Dropdown conversions

## Steps

1. Read both files end-to-end. Inventory controls.

2. For each file, replace any local `seg` / `segV` definition with an import
   from `./segStyles.js`. If a file has no helper and uses inline classes
   directly, migrate the inline classes into `seg()` / `segV()` call sites.

3. Change the form-root `space-y-*` to `space-y-3` in each file.

4. Swap any `<div class="flex flex-col">` wrapping vertical stacked lists to
   `<div class="inline-flex flex-col">`.

5. For every native `<input type="checkbox">`: apply `class="fade-check"` and
   wrap in the canonical tile `<label>`.

6. For any `<input type="range">`: add `class="fade-range"` + inline
   `--fade-range-pct` style.

7. Migrate any inline active-state class strings
   (`bg-[var(--accent)] text-white border-[var(--accent)]`) to `'seg-active z-10'`.

8. Swap any inline `color-mix(... #000 40%)` "hint panel" background to
   `var(--surface-hint)` where the intent matches.

9. `npx vite build`. Confirm exit 0.

10. Visual spot-check: in the running dev app, pick an archive format (ZIP,
    TAR) and a data format (JSON, CSV, …) — compare with MP4 for visual
    parity on shared controls.

## Success signal
- Both files: no inline active-button class strings; all active states flow
  through `'seg-active z-10'` via the shared helpers.
- Checkboxes use `fade-check` inside `bg-[var(--surface-hint)]` tiles.
- Range sliders (if any) use `fade-range` + pct style.
- `space-y-3` on form roots, `py-[5px]` on segmented buttons.
- `npx vite build` succeeds.
- Visual parity with MP4 on shared control types.
- Summary lists anything deferred.
