# TASK-2: Apply unified polish patterns to AudioOptions.svelte

## Goal
`src/lib/AudioOptions.svelte` renders with the same visual polish as the MP4
panel: unified section spacing, shrunk segmented buttons, new active-button
style, custom checkboxes in gray tiles, tightened range sliders, vertical
stacked lists shrunk to longest-row width. No behavioral changes.

## Context

Fade is a Tauri 2 + Svelte 5 media-conversion app. We just finished polishing
the MP4/Video output panel (`src/lib/VideoOptions.svelte`) and landed on a
set of canonical patterns. We're now propagating them to the sibling option
panels. AudioOptions is the largest sibling and has the most surfaces.

**Canonical primitives already in place** (defined in `src/app.css`):
- `.seg-inactive` — bevel-gradient background, bottom-anchored
- `.seg-active` — 50%-darker accent fill + 2px accent underline inside the
  bottom edge with 10px gutters, no 1px outline, applied alongside `z-10`
- `.fade-check` — custom checkbox (14px rounded square, dark-grey/white
  border unchecked; accent-blue fill with tiny white × when checked)
- `.fade-range` — slim 2px slider with accent fill from 0 to `--fade-range-pct`
- `.fade-scroll-stable` — always-reserved 10px scrollbar gutter
- `--surface-hint` token — the standard "background dark gray" for gray tiles

**Canonical class strings** (used by `VideoOptions.svelte` today):
- Segmented row button: `px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative` + connected corners (`rounded-l-md` / `rounded-r-md` first/last, `-ml-px` overlap after index 0)
- Vertical segmented button: `w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative` + (`rounded-t-md` / `rounded-b-md`, `-mt-px`)
- Active variant: `'seg-active z-10'`
- Inactive variant: `'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'`
- Form root: `<div class="space-y-3" role="form" ...>` (was `space-y-3.5`)
- Vertical stacked list wrapper: `<div class="inline-flex flex-col">` (was `flex flex-col`) — shrinks to longest-row width
- Checkbox tile: `<label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px] bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2 {checked ? 'text-[var(--text-primary)]' : 'text-white/75'}">`
- Range slider: `<input type="range" ... class="fade-range" style="--fade-range-pct:{(v-min)/(max-min)*100}%" />` (style binding makes the blue fill track the thumb)

TASK-1 extracted `seg()` / `segV()` into `src/lib/segStyles.js`. Import from
there rather than redefining or using the local copies in AudioOptions.

**Judgment calls that are explicitly deferred** — flag in the summary rather
than deciding alone:
- Dropdown-overlay rewrites (if AudioOptions has any `overlay.svelte.js`-based
  dropdown like the old codec menu, leave it — conversion to inline absolute
  menus is deferred).
- Slider-label designs (the CRF "Extreme quality / High quality / Optimized /
  Low quality" pattern is specific to CRF. Do NOT invent label buckets for
  other sliders. Apply the `.fade-range` fill + padding only).
- Checkbox "tile grouping" — if multiple checkboxes appear next to each
  other, wrap each individually; don't try to merge them visually.

## In scope
- `src/lib/AudioOptions.svelte` — all visual updates listed above

## Out of scope
- Any other `.svelte` file
- `src/app.css`
- Behavior changes — no new options, no changed defaults, no renamed bindings
- Adding slider labels (CRF-quality-style text)
- Converting any existing dropdown to the inline-absolute pattern
- The unused `devSeg` / `devSegV` helpers — leave them
- Changing the option schema or emitted values

## Steps

1. Read `src/lib/AudioOptions.svelte` end-to-end. Map out every section:
   fieldsets, checkboxes, sliders, dropdowns, vertical stacked lists.

2. Remove the local `seg`, `segV`, and `segGrid` helper definitions **only if
   they are byte-for-byte replaceable** by the shared helper. If `segGrid` has
   unique behavior the shared helpers don't cover, keep `segGrid` local and
   only replace `seg` / `segV`. Add `import { seg, segV } from './segStyles.js';`.

3. Change the form-root `space-y-*` to `space-y-3`.

4. Wherever the root form has `<div class="flex flex-col">` around a vertical
   stacked list of segmented buttons, change to `<div class="inline-flex flex-col">`.

5. For every native `<input type="checkbox" ...>`:
   - Add `class="fade-check"` (replacing any existing `accent-*` or no class).
   - Wrap the surrounding `<label>` (or create one) with the checkbox-tile
     classes from the context section. Use the relevant boolean as the
     checked-state key for the text-color toggle.

6. For every `<input type="range" ...>`:
   - Add `class="fade-range"` if missing.
   - Add the inline `style="--fade-range-pct:{(value-min)/(max-min)*100}%"`.
     Use the actual bound variable and the element's `min`/`max` attrs. No
     added label text.

7. For segmented-button rows/columns that still have inline active/inactive
   class strings (not going through `seg()` / `segV()`), migrate them to use
   the helpers. If a button group doesn't fit the helpers' signature, leave
   it and flag it in the summary.

8. Run `npx vite build`. Confirm exit 0 with no new warnings for this file.

9. Open the app (`npm run tauri dev` if not already running on :1427), select
   an audio output format (e.g. MP3), and visually compare against the MP4
   panel. Spot-check: button heights, active-underline appearance, checkbox
   tile color, slider fill color, section spacing.

## Success signal
- `src/lib/AudioOptions.svelte` no longer contains any inline
  `'bg-[var(--accent)] text-white border-[var(--accent)]'` active-button
  class strings — all active states go through `seg()` / `segV()` which
  return `'seg-active z-10'`.
- Every `<input type="checkbox">` uses `fade-check` and sits inside a tile
  with `bg-[var(--surface-hint)]`.
- Every `<input type="range">` has `fade-range` + the inline `--fade-range-pct`
  style binding.
- `npx vite build` succeeds.
- Visual parity with the MP4 panel on equivalent controls — same button
  height, same underline, same tile color, same slider fill.
- Summary lists any spots that were deferred (dropdowns, unusual button
  groups, ambiguous tile groupings).

## Notes
- The previous conversation also standardized replacing inline `color-mix`
  "hint-panel" backgrounds with `var(--surface-hint)`. If AudioOptions has
  any such inline color, swap to the token.
- If the file currently uses `py-1.5` for segmented buttons, the canonical
  is `py-[5px]` (2 px shorter, font size unchanged).
