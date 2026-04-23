# TASK-3: Apply unified polish patterns to ImageOptions.svelte

## Goal
`src/lib/ImageOptions.svelte` renders with the same visual polish as the MP4
panel: shared segmented-button helper, canonical spacing, shrunk button
heights, new active-button style, custom checkboxes in gray tiles, and
tightened range sliders. No behavioral changes.

## Context

Fade is a Tauri 2 + Svelte 5 media-conversion app. We just finished polishing
the MP4/Video output panel and are propagating the canonical patterns to the
sibling option panels. ImageOptions is the second-largest sibling.

**Canonical primitives already in place** (defined in `src/app.css`):
- `.seg-inactive` — bevel-gradient background, bottom-anchored
- `.seg-active` — 50%-darker accent fill + 2px accent underline inside the
  bottom edge with 10px gutters, no 1px outline
- `.fade-check` — custom checkbox (dark-grey rounded square; accent-blue with
  tiny white × when checked)
- `.fade-range` — slim 2px slider with accent fill driven by `--fade-range-pct`
- `--surface-hint` token — the standard "background dark gray"

**Canonical class strings** (match `VideoOptions.svelte`):
- Segmented row button: `px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative` + `rounded-l-md` / `rounded-r-md` + `-ml-px` overlap
- Vertical segmented button: `w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative` + `rounded-t-md` / `rounded-b-md` + `-mt-px`
- Active variant: `'seg-active z-10'`
- Inactive variant: `'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'`
- Form root: `<div class="space-y-3" role="form" ...>`
- Vertical list wrapper: `<div class="inline-flex flex-col">` (shrinks to longest row)
- Checkbox tile: `<label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px] bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2 {checked ? 'text-[var(--text-primary)]' : 'text-white/75'}">`
- Range slider: `<input type="range" ... class="fade-range" style="--fade-range-pct:{(v-min)/(max-min)*100}%" />`

TASK-1 extracted the canonical `seg()` / `segV()` into `src/lib/segStyles.js`.
Import from there.

**Deferred judgment calls** (flag in summary):
- Slider-label designs — do NOT invent CRF-style quality labels for image
  quality sliders. Apply the `.fade-range` fill only.
- Dropdown overlay conversions — leave any existing dropdowns as-is.
- Crop-aspect ratio buttons or any bespoke image-specific buttons that don't
  fit `seg()` / `segV()` — leave them, flag them.

## In scope
- `src/lib/ImageOptions.svelte` only

## Out of scope
- Any other `.svelte` file
- `src/app.css`
- Behavior / schema changes
- Adding slider labels
- Dropdown conversions
- The unused `devSeg` / `devSegV` helpers — leave them
- Any App.svelte integration (crop handlers, quality callbacks, etc.)

## Steps

1. Read `src/lib/ImageOptions.svelte` top to bottom. Inventory the control
   types: segmented rows/columns, checkboxes, sliders, anything else.

2. Replace the local `seg` / `segV` helper definitions with an import from
   `./segStyles.js`. Leave `devSeg` / `devSegV` alone.

3. Change the form-root `space-y-*` to `space-y-3`.

4. Any `<div class="flex flex-col">` wrapping a vertical segmented list →
   `<div class="inline-flex flex-col">`.

5. Native checkboxes: apply `class="fade-check"` and wrap in a tile `<label>`
   using the canonical classes. Bind the text-color conditional to the
   checked state.

6. Range sliders: add `class="fade-range"` and the inline
   `style="--fade-range-pct:{(value-min)/(max-min)*100}%"` using the bound
   variable and min/max attrs.

7. Migrate any inline active-state class strings (`bg-[var(--accent)]
   text-white border-[var(--accent)]`) to the `seg()` helper so they return
   `'seg-active z-10'`.

8. Replace any inline `color-mix(in srgb, ... #000 40%)` "hint panel"
   backgrounds with `var(--surface-hint)` where the intent matches (dim
   recessed surface). Do not swap unrelated `color-mix` usages.

9. `npx vite build`. Confirm exit 0.

10. Visual spot-check: launch or reload the dev app, pick an image format
    (PNG, JPG, WEBP), compare against the MP4 panel for parity on shared
    controls.

## Success signal
- No inline active-button class strings remain; all active states flow
  through `seg()` / `segV()` returning `'seg-active z-10'`.
- Every `<input type="checkbox">` uses `fade-check` inside a
  `bg-[var(--surface-hint)]` tile.
- Every `<input type="range">` uses `fade-range` + the inline pct style.
- Button height is `py-[5px]`, section spacing is `space-y-3`.
- `npx vite build` succeeds.
- Visual parity with the MP4 panel on all shared controls.
- Summary lists anything deferred (sliders without labels, dropdowns, bespoke
  buttons).
